//! Reminders (§12.5b): Dart's notification adapter subscribes to the ops stream, applies each
//! op with `flutter_local_notifications` and reports the result; notification actions come
//! back as intents.

use super::lift;
use super::runtime::{self, core};
use super::sink::DartSink;
use crate::frb_generated::StreamSink;
use crate::sync::engine::Trigger;
use crate::view::model::CoreFailure;
use crate::view::model::{NotificationAction, NotificationOp, NotificationResult};

/// The notification-ops stream (schedule / update / cancel / show now). It does not depend on
/// a session: while signed out it stays open and receives nothing; the ops of whichever account
/// signs in arrive on it.
pub fn watch_notification_ops(sink: StreamSink<NotificationOp>) -> Result<(), CoreFailure> {
    lift(|| {
        let core = core()?;
        core.env().notifications.attach(Box::new(DartSink(sink)));
        if let Ok(session) = core.session() {
            session.flush_notifications();
            session.recompute_notifications()?;
        }
        Ok(())
    })
}

/// The platform's result of an op.
pub fn report_notification_result(id: i32, result: NotificationResult) -> Result<(), CoreFailure> {
    lift(|| core()?.session()?.notification_result(id, result))
}

/// A notification action (Done / Snooze), applied through the outbox.
pub fn notification_action(id: i32, action: NotificationAction) -> Result<String, CoreFailure> {
    lift(|| {
        let op = core()?.session()?.notification_action(id, action)?;
        runtime::trigger(Trigger::AfterWrite);
        Ok(op)
    })
}
