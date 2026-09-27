import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:strata/src/reminders/background.dart';
import 'package:strata/src/reminders/notification_platform.dart';
import 'package:strata_state/strata_state.dart';
// `TZDateTime` is only used to hand the core's UTC instant to
// `zonedSchedule` unchanged (no time maths); `timezone` comes with
// flutter_local_notifications.
// ignore: depend_on_referenced_packages
import 'package:timezone/timezone.dart' as tz;

/// [NotificationPlatform] on `flutter_local_notifications` (PLAN §11.1,
/// D27): Android channel + POST_NOTIFICATIONS and exact-alarm permission
/// requests, iOS/macOS permissions and the Done/Snooze category, Linux and
/// Windows settings.
class LocalNotificationsPlatform implements NotificationPlatform {
  /// Creates the platform over [plugin].
  new({FlutterLocalNotificationsPlugin? plugin})
    : _plugin = plugin ?? FlutterLocalNotificationsPlugin();

  final FlutterLocalNotificationsPlugin _plugin;
  NotificationDetails? _details;

  static const String _channelId = 'reminders';
  static const String _category = 'reminder';

  @override
  Future<void> initialize({
    required NotificationStrings strings,
    required ValueChanged<NotificationTap> onTap,
  }) async {
    final darwin = DarwinInitializationSettings(
      requestAlertPermission: false,
      requestBadgePermission: false,
      requestSoundPermission: false,
      notificationCategories: [
        DarwinNotificationCategory(
          _category,
          actions: [
            DarwinNotificationAction.plain(ReminderActions.done, strings.done),
            DarwinNotificationAction.plain(
              ReminderActions.snooze,
              strings.snooze,
            ),
          ],
        ),
      ],
    );
    await _plugin.initialize(
      settings: InitializationSettings(
        android: const AndroidInitializationSettings('@mipmap/ic_launcher'),
        iOS: darwin,
        macOS: darwin,
        linux: LinuxInitializationSettings(defaultActionName: strings.open),
        windows: const WindowsInitializationSettings(
          appName: 'Strata',
          appUserModelId: 'app.strata.Strata',
          guid: '7c1f7f3e-3a2f-4f38-9d55-6d0f5f0f2a51',
        ),
      ),
      onDidReceiveNotificationResponse: (response) => onTap(tapOf(response)),
      onDidReceiveBackgroundNotificationResponse: onBackgroundNotification,
    );
    _details = NotificationDetails(
      android: AndroidNotificationDetails(
        _channelId,
        strings.channelName,
        channelDescription: strings.channelDescription,
        importance: Importance.high,
        priority: Priority.high,
        category: AndroidNotificationCategory.reminder,
        actions: [
          AndroidNotificationAction(ReminderActions.done, strings.done),
          AndroidNotificationAction(ReminderActions.snooze, strings.snooze),
        ],
      ),
      iOS: const DarwinNotificationDetails(categoryIdentifier: _category),
      macOS: const DarwinNotificationDetails(categoryIdentifier: _category),
      linux: LinuxNotificationDetails(
        actions: [
          LinuxNotificationAction(
            key: ReminderActions.done,
            label: strings.done,
          ),
          LinuxNotificationAction(
            key: ReminderActions.snooze,
            label: strings.snooze,
          ),
        ],
      ),
      windows: WindowsNotificationDetails(
        actions: [
          WindowsAction(content: strings.done, arguments: ReminderActions.done),
          WindowsAction(
            content: strings.snooze,
            arguments: ReminderActions.snooze,
          ),
        ],
      ),
    );
    await _requestPermissions(strings);
  }

  Future<void> _requestPermissions(NotificationStrings strings) async {
    final android = _plugin
        .resolvePlatformSpecificImplementation<
          AndroidFlutterLocalNotificationsPlugin
        >();
    if (android != null) {
      await android.createNotificationChannel(
        AndroidNotificationChannel(
          _channelId,
          strings.channelName,
          description: strings.channelDescription,
          importance: Importance.high,
        ),
      );
      await android.requestNotificationsPermission();
      if (await android.canScheduleExactNotifications() != true) {
        await android.requestExactAlarmsPermission();
      }
    }
    await _plugin
        .resolvePlatformSpecificImplementation<
          IOSFlutterLocalNotificationsPlugin
        >()
        ?.requestPermissions(alert: true, badge: true, sound: true);
    await _plugin
        .resolvePlatformSpecificImplementation<
          MacOSFlutterLocalNotificationsPlugin
        >()
        ?.requestPermissions(alert: true, badge: true, sound: true);
  }

  /// The platform's permission answer before scheduling (Android reports
  /// notifications and exact alarms separately).
  Future<bool> _permitted() async {
    final android = _plugin
        .resolvePlatformSpecificImplementation<
          AndroidFlutterLocalNotificationsPlugin
        >();
    if (android == null) return true;
    return await android.areNotificationsEnabled() != false &&
        await android.canScheduleExactNotifications() != false;
  }

  @override
  Future<NotificationTap?> launchTap() async {
    final details = await _plugin.getNotificationAppLaunchDetails();
    final response = details?.notificationResponse;
    if (details == null || !details.didNotificationLaunchApp) return null;
    return response == null ? null : tapOf(response);
  }

  @override
  Future<NotificationResult> schedule({
    required int id,
    required DateTime at,
    required String title,
    required String body,
    required String payload,
  }) => _run(() async {
    if (!await _permitted()) return NotificationResult.permissionDenied;
    await _plugin.zonedSchedule(
      id: id,
      scheduledDate: tz.TZDateTime.from(at, tz.UTC),
      notificationDetails: _details!,
      androidScheduleMode: AndroidScheduleMode.exactAllowWhileIdle,
      title: title,
      body: body,
      payload: payload,
    );
    return NotificationResult.ok;
  });

  @override
  Future<NotificationResult> show({
    required int id,
    required String title,
    required String body,
    required String payload,
  }) => _run(() async {
    await _plugin.show(
      id: id,
      title: title,
      body: body,
      notificationDetails: _details,
      payload: payload,
    );
    return NotificationResult.ok;
  });

  @override
  Future<NotificationResult> cancel({required int id}) => _run(() async {
    await _plugin.cancel(id: id);
    return NotificationResult.ok;
  });

  /// Maps the plugin's platform errors to the core's results.
  Future<NotificationResult> _run(
    Future<NotificationResult> Function() call,
  ) async {
    try {
      return await call();
    } on PlatformException catch (error) {
      return error.code == 'exact_alarms_not_permitted' ||
              error.code == 'permissionDenied'
          ? NotificationResult.permissionDenied
          : NotificationResult.platformLimit;
    }
  }
}

/// A plugin response as a [NotificationTap].
NotificationTap tapOf(NotificationResponse response) => NotificationTap(
  id: response.id ?? 0,
  actionId: response.actionId,
  payload: response.payload,
);
