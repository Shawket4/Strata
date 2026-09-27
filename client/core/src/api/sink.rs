//! Dart streams as view sinks.

use crate::frb_generated::{SseEncode, StreamSink};
use crate::view::ViewSink;

/// A Dart stream receiving view-models; closed when Dart cancels the subscription.
pub(crate) struct DartSink<T>(pub(crate) StreamSink<T>);

impl<T: SseEncode + Send + Sync> ViewSink<T> for DartSink<T> {
    fn emit(&self, value: T) -> bool {
        self.0.add(value).is_ok()
    }
}
