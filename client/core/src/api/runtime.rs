//! The process-wide core and its background loop (sync triggers, timers, reminders).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::{Notify, mpsc};

use crate::error::{CoreError, CoreResult};
use crate::session::Core;
use crate::store::sync_state;
use crate::sync::engine::{Trigger, backoff_delay};

/// Foreground pull interval (§12.4 "on a timer while foregrounded").
const TIMER: Duration = Duration::from_secs(60);

struct Runtime {
    core: Arc<Core>,
    trigger: Option<mpsc::UnboundedSender<Trigger>>,
    /// Asks the `/events` loop to drop its connection and subscribe again at once.
    reconnect: Arc<Notify>,
}

static RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Runtime>> {
    RUNTIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The installed core.
pub(crate) fn core() -> CoreResult<Arc<Core>> {
    lock()
        .as_ref()
        .map(|r| r.core.clone())
        .ok_or(CoreError::NotInitialised)
}

/// Installs a core (replacing any previous one) and, inside a Tokio runtime, starts its
/// background loop and its `/events` subscription.
pub(crate) fn install(core: Core) -> Arc<Core> {
    let core = Arc::new(core);
    let reconnect = Arc::new(Notify::new());
    let trigger = tokio::runtime::Handle::try_current().ok().map(|handle| {
        let (tx, rx) = mpsc::unbounded_channel();
        handle.spawn(background(core.clone(), rx));
        handle.spawn(events_loop(core.clone(), tx.clone(), reconnect.clone()));
        tx
    });
    *lock() = Some(Runtime {
        core: core.clone(),
        trigger,
        reconnect,
    });
    core
}

/// Makes the `/events` loop reconnect now (pull-to-refresh: a connection that silently
/// dropped comes back without waiting for its backoff).
pub(crate) fn reconnect_events() {
    if let Some(r) = lock().as_ref() {
        r.reconnect.notify_one();
    }
}

/// Wakes the background loop.
pub(crate) fn trigger(t: Trigger) {
    if let Some(tx) = lock().as_ref().and_then(|r| r.trigger.clone()) {
        let _ = tx.send(t);
    }
}

fn next_delay(core: &Core) -> Duration {
    let Ok(session) = core.session() else {
        return TIMER;
    };
    let failures = session
        .read(|c, _| Ok(sync_state::get(c)?.consecutive_failures))
        .unwrap_or(0);
    let mut delay = if failures > 0 {
        backoff_delay(failures)
    } else {
        TIMER
    };
    // Linux: wake up for the next reminder while the app runs (§12.5b).
    if let Ok(Some(at)) = session.tick_notifications() {
        let now = core.env().clock.now();
        let until = (at - now).to_std().unwrap_or(Duration::ZERO);
        delay = delay.min(until);
    }
    delay
}

async fn background(core: Arc<Core>, mut rx: mpsc::UnboundedReceiver<Trigger>) {
    let mut trigger = Trigger::Start;
    loop {
        if let Ok(session) = core.session() {
            if let Err(e) = session.sync(trigger).await {
                tracing::warn!("sync cycle failed: {e}");
            }
            if let Err(e) = session.recompute_notifications() {
                tracing::warn!("reminder plan failed: {e}");
            }
            // An account without a time zone of its own takes the device's.
            if let Err(e) = core.adopt_device_timezone().await {
                tracing::warn!("adopting the device time zone failed: {e}");
            }
        }
        if let Err(e) = core.publish_state() {
            tracing::warn!("session state failed: {e}");
        }
        let delay = next_delay(&core);
        trigger = tokio::select! {
            t = rx.recv() => match t {
                Some(t) => t,
                None => return,
            },
            () = tokio::time::sleep(delay) => Trigger::Timer,
        };
    }
}

/// The `/events` subscription of the active session (§12.4: events only trigger pulls):
/// resumes from the saved seq, pulls on every change or reset, applies `account.disabled`,
/// re-subscribes with backoff after a drop and whenever the active session changes, and at
/// once when asked to ([`reconnect_events`]).
async fn events_loop(core: Arc<Core>, tx: mpsc::UnboundedSender<Trigger>, reconnect: Arc<Notify>) {
    let mut failures = 0u32;
    'subscribe: loop {
        let generation = core.generation();
        let Ok(session) = core.session() else {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };
        match session.subscribe_events() {
            Err(_) => failures += 1,
            Ok(mut stream) => loop {
                tokio::select! {
                    next = stream.next() => match next {
                        Some(Ok(signal)) => {
                            failures = 0;
                            match session.handle_event(&signal) {
                                Ok(true) => {
                                    if tx.send(Trigger::EventsFrame).is_err() {
                                        return;
                                    }
                                }
                                Ok(false) => {
                                    let _ = core.publish_state();
                                    break;
                                }
                                Err(e) => tracing::warn!("event failed: {e}"),
                            }
                        }
                        Some(Err(e)) => {
                            failures += 1;
                            let _ = session.account_failure(&e);
                            let _ = core.publish_state();
                            break;
                        }
                        None => break,
                    },
                    () = reconnect.notified() => {
                        failures = 0;
                        drop(stream);
                        drop(session);
                        continue 'subscribe;
                    }
                    () = tokio::time::sleep(Duration::from_secs(1)) => {
                        if core.generation() != generation {
                            break;
                        }
                    }
                }
            },
        }
        drop(session);
        tokio::select! {
            () = tokio::time::sleep(backoff_delay(failures.max(1))) => {}
            () = reconnect.notified() => failures = 0,
        }
    }
}
