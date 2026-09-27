import 'package:flutter/widgets.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:strata/src/boot/core_bootstrap.dart';
import 'package:strata/src/reminders/local_notifications_platform.dart';
import 'package:strata/src/reminders/notification_platform.dart';
import 'package:strata/src/reminders/reminder_adapter.dart';
import 'package:strata_state/strata_state.dart';

/// A notification action tapped while the app was in the background
/// (Android/iOS run this in a background isolate; PLAN §12.5b "Done and
/// Snooze launch the app in the background"): opens the core and forwards
/// the action.
@pragma('vm:entry-point')
Future<void> onBackgroundNotification(NotificationResponse response) async {
  WidgetsFlutterBinding.ensureInitialized();
  await forwardBackgroundTap(
    tapOf(response),
    bootstrap: const NativeCoreBootstrap(),
    core: const BridgeCoreApi(),
  );
}

/// Opens the core with [bootstrap] and forwards a Done/Snooze [tap] to
/// `notificationAction`; a plain tap needs the UI and is left to the
/// foreground launch.
Future<void> forwardBackgroundTap(
  NotificationTap tap, {
  required CoreBootstrap bootstrap,
  required CoreApi core,
}) async {
  final action = actionOf(tap);
  if (action == null) return;
  await bootstrap.loadLibrary();
  await core.initCore(config: await bootstrap.config());
  await core.notificationAction(id: tap.id, action: action);
}
