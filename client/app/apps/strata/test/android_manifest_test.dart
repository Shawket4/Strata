// Reads the platform folders of this package: the test runs with the package
// root as its working directory.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// What flutter_local_notifications needs on Android to schedule reminders,
/// keep them across reboots and handle Done / Snooze (its README, Android
/// setup). Without `SCHEDULE_EXACT_ALARM` Android greys out the app's
/// "Alarms & reminders" switch, so the exact-alarm request cannot be granted.
void main() {
  final manifest = File(
    'android/app/src/main/AndroidManifest.xml',
  ).readAsStringSync();

  test('declares the reminder permissions', () {
    for (final permission in [
      'android.permission.INTERNET',
      'android.permission.POST_NOTIFICATIONS',
      'android.permission.SCHEDULE_EXACT_ALARM',
      'android.permission.RECEIVE_BOOT_COMPLETED',
    ]) {
      expect(
        manifest,
        contains('<uses-permission android:name="$permission"/>'),
        reason: permission,
      );
    }
  });

  test('registers the plugin receivers', () {
    for (final receiver in [
      'ScheduledNotificationReceiver',
      'ScheduledNotificationBootReceiver',
      'ActionBroadcastReceiver',
    ]) {
      expect(
        manifest,
        contains(
          'android:name="com.dexterous.flutterlocalnotifications.$receiver"',
        ),
        reason: receiver,
      );
    }
    for (final action in [
      'android.intent.action.BOOT_COMPLETED',
      'android.intent.action.MY_PACKAGE_REPLACED',
      'android.intent.action.QUICKBOOT_POWERON',
    ]) {
      expect(manifest, contains('<action android:name="$action"/>'));
    }
  });
}
