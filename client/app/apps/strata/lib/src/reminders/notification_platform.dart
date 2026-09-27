import 'package:flutter/foundation.dart';
import 'package:strata_state/strata_state.dart';

/// Action identifiers of the reminder notifications (Done / Snooze).
abstract final class ReminderActions {
  /// Marks the task done.
  static const String done = 'done';

  /// Snoozes the reminder.
  static const String snooze = 'snooze';
}

/// A tap on a reminder notification or one of its actions.
@immutable
final class NotificationTap {
  /// Creates a tap.
  const new({required this.id, this.actionId, this.payload});

  /// The notification's stable ID (from the core's op).
  final int id;

  /// `done`, `snooze`, or `null` for the notification itself.
  final String? actionId;

  /// The task ID the op carried.
  final String? payload;

  @override
  bool operator ==(Object other) =>
      other is NotificationTap &&
      other.id == id &&
      other.actionId == actionId &&
      other.payload == payload;

  @override
  int get hashCode => Object.hash(id, actionId, payload);

  @override
  String toString() => 'NotificationTap($id, $actionId, $payload)';
}

/// Localised copy the platform shows (action labels, Android channel).
@immutable
final class NotificationStrings {
  /// Creates the strings.
  const new({
    required this.done,
    required this.snooze,
    required this.open,
    required this.channelName,
    required this.channelDescription,
  });

  /// "Done" action.
  final String done;

  /// "Snooze" action.
  final String snooze;

  /// Default action name (Linux).
  final String open;

  /// Android channel name.
  final String channelName;

  /// Android channel description.
  final String channelDescription;
}

/// The thin seam over `flutter_local_notifications` the reminders adapter
/// talks to (PLAN §12.5b): one call per core op, the platform's answer as a
/// [NotificationResult]. Tests use a fake.
abstract interface class NotificationPlatform {
  /// Initialises the plugin for this platform (channels, categories,
  /// permission requests) and routes taps to [onTap].
  Future<void> initialize({
    required NotificationStrings strings,
    required ValueChanged<NotificationTap> onTap,
  });

  /// The tap that launched the app, if any.
  Future<NotificationTap?> launchTap();

  /// `zonedSchedule(id, at, …)`; the same ID replaces a scheduled one.
  Future<NotificationResult> schedule({
    required int id,
    required DateTime at,
    required String title,
    required String body,
    required String payload,
  });

  /// Shows a notification now (Linux `show_now`).
  Future<NotificationResult> show({
    required int id,
    required String title,
    required String body,
    required String payload,
  });

  /// `cancel(id)`.
  Future<NotificationResult> cancel({required int id});
}
