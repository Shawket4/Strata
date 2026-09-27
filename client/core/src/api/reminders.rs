//! Reminders (§12.5b): Dart's notification adapter subscribes to the ops stream, applies each
//! op with `flutter_local_notifications` and reports the result; notification actions come
//! back as intents.

use super::runtime::{self, core};
use super::lift;
use super::sink::DartSink;
use crate::view::model::CoreFailure;
use crate::frb_generated::StreamSink;
use crate::sync::engine::Trigger;
use crate::view::model::{NotificationAction, NotificationOp, NotificationResult};

/// The notification-ops stream (schedule / update / cancel / show now).
pub fn watch_notification_ops(sink: StreamSink<NotificationOp>) -> Result<(), CoreFailure> {
    lift(|| {
        let session = core()?.session()?;
        session.attach_notifications(Box::new(DartSink(sink)));
        session.recompute_notifications()
    })
}

/// The platform's result of an op.
pub fn report_notification_result(id: i32, result: NotificationResult) -> Result<(), CoreFailure> {
    lift(|| {
        core()?.session()?.notification_result(id, result)
    })
}

/// A notification action (Done / Snooze), applied through the outbox.
pub fn notification_action(id: i32, action: NotificationAction) -> Result<String, CoreFailure> {
    lift(|| {
        let op = core()?.session()?.notification_action(id, action)?;
        runtime::trigger(Trigger::AfterWrite);
        Ok(op)
    })
}
