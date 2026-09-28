import 'package:flutter/foundation.dart';

/// Reports an error the core did not type (anything but a `CoreFailure`:
/// an FRB-surfaced panic, an unmapped transport error) to Flutter's error
/// reporting, so it reaches the logs instead of vanishing. Only the error,
/// its stack and the [action] ("signing in") are reported, never what the
/// user typed. The screen shows a generic message itself.
void reportUntypedFailure(
  Object error,
  StackTrace stack, {
  required String action,
}) => FlutterError.reportError(
  FlutterErrorDetails(
    exception: error,
    stack: stack,
    library: 'strata',
    context: ErrorDescription('while $action'),
  ),
);
