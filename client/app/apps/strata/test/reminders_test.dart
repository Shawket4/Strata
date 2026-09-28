import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/strata.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/boot.dart';
import 'helpers/fakes.dart';
import 'helpers/matrix.dart';

final DateTime _at = DateTime.utc(2026, 9, 27, 7);

NotificationOp _op(NotificationOpKind kind, {int id = 4211, DateTime? at}) =>
    NotificationOp(
      kind: kind,
      id: id,
      at: at,
      title: 'Petrol Arrows invoice',
      body: 'due today 10:00',
      taskId: 't-petrol-arrows',
    );

const _schedule = PlatformCall('schedule', {
  'id': 4211,
  'at': null,
  'title': 'Petrol Arrows invoice',
  'body': 'due today 10:00',
  'payload': 't-petrol-arrows',
});

PlatformCall _scheduled(DateTime at) =>
    PlatformCall('schedule', {..._schedule.args, 'at': at});

const _shown = PlatformCall('show', {
  'id': 4211,
  'title': 'Petrol Arrows invoice',
  'body': 'due today 10:00',
  'payload': 't-petrol-arrows',
});

void main() {
  group('ReminderAdapter', () {
    late FakeCoreApi core;
    late FakeNotificationPlatform platform;
    late List<String> opened;
    late ReminderAdapter adapter;

    setUp(() {
      core = FakeCoreApi();
      platform = FakeNotificationPlatform();
      opened = [];
      adapter = ReminderAdapter(
        core: core,
        platform: platform,
        onOpenTask: opened.add,
      );
    });

    test('schedule → zonedSchedule at the core instant, reports ok', () async {
      await adapter.apply(_op(NotificationOpKind.schedule, at: _at));
      expect(platform.calls, [_scheduled(_at)]);
      expect(core.calls, [
        const CoreCall('reportNotificationResult', {
          'id': 4211,
          'result': NotificationResult.ok,
        }),
      ]);
    });

    test('update replaces the same ID', () async {
      final later = _at.add(const Duration(hours: 1));
      await adapter.apply(_op(NotificationOpKind.update, at: later));
      expect(platform.calls, [_scheduled(later)]);
    });

    test('cancel → cancel(id)', () async {
      await adapter.apply(_op(NotificationOpKind.cancel));
      expect(platform.calls, [
        const PlatformCall('cancel', {'id': 4211}),
      ]);
      expect(core.calls.single.method, 'reportNotificationResult');
    });

    test('show now (Linux) → show', () async {
      await adapter.apply(_op(NotificationOpKind.showNow));
      expect(platform.calls, [_shown]);
    });

    test('a schedule without an instant is shown now', () async {
      await adapter.apply(_op(NotificationOpKind.schedule));
      expect(platform.calls, [_shown]);
    });

    for (final result in [
      NotificationResult.permissionDenied,
      NotificationResult.platformLimit,
    ]) {
      test('reports $result', () async {
        platform.result = result;
        await adapter.apply(_op(NotificationOpKind.schedule, at: _at));
        expect(core.calls, [
          CoreCall('reportNotificationResult', {'id': 4211, 'result': result}),
        ]);
      });
    }

    test('ops run in the core order', () async {
      final a = adapter.enqueue(_op(NotificationOpKind.cancel, id: 1));
      final b = adapter.enqueue(
        _op(NotificationOpKind.schedule, id: 1, at: _at),
      );
      await Future.wait([a, b]);
      expect(platform.calls.map((c) => c.method), ['cancel', 'schedule']);
      expect(core.calls.map((c) => c.args['id']), [1, 1]);
    });

    test('Done and Snooze go to notificationAction', () async {
      await adapter.handleTap(
        const NotificationTap(
          id: 4211,
          actionId: ReminderActions.done,
          payload: 't-petrol-arrows',
        ),
      );
      await adapter.handleTap(
        const NotificationTap(id: 4211, actionId: ReminderActions.snooze),
      );
      expect(core.calls, [
        const CoreCall('notificationAction', {
          'id': 4211,
          'action': NotificationAction(kind: NotificationActionKind.done),
        }),
        const CoreCall('notificationAction', {
          'id': 4211,
          'action': NotificationAction(kind: NotificationActionKind.snooze),
        }),
      ]);
      expect(opened, isEmpty);
    });

    test('a tap on the notification opens its task', () async {
      await adapter.handleTap(
        const NotificationTap(id: 4211, payload: 't-petrol-arrows'),
      );
      await adapter.handleTap(const NotificationTap(id: 1, payload: ''));
      await adapter.handleTap(const NotificationTap(id: 2));
      expect(opened, ['t-petrol-arrows']);
      expect(core.calls, isEmpty);
    });
  });

  group('background taps', () {
    test('Done opens the core and forwards the action', () async {
      final core = FakeCoreApi();
      final bootstrap = FakeBootstrap();
      await forwardBackgroundTap(
        const NotificationTap(id: 7, actionId: ReminderActions.done),
        bootstrap: bootstrap,
        core: core,
      );
      expect(bootstrap.loads, 1);
      expect(core.calls, [
        const CoreCall('initCore', {'config': StrataFixtures.coreConfig}),
        const CoreCall('notificationAction', {
          'id': 7,
          'action': NotificationAction(kind: NotificationActionKind.done),
        }),
      ]);
    });

    test('a plain tap waits for the foreground launch', () async {
      final core = FakeCoreApi();
      final bootstrap = FakeBootstrap();
      await forwardBackgroundTap(
        const NotificationTap(id: 7, payload: 't-1'),
        bootstrap: bootstrap,
        core: core,
      );
      expect(bootstrap.loads, 0);
      expect(core.calls, isEmpty);
    });
  });

  group('adapter in the app', () {
    testWidgets('initialises the platform with localised copy', (tester) async {
      final app = await boot(tester);
      expect(app.platform.calls.first, const PlatformCall('initialize'));
      expect(app.platform.strings?.done, 'Done');
      expect(app.platform.strings?.snooze, 'Snooze');
      expect(app.platform.strings?.channelName, 'Reminders');
      expect(app.fake.calls, contains(const CoreCall('watchNotificationOps')));
    });

    testWidgets('streamed ops reach the plugin and results the core', (
      tester,
    ) async {
      final app = await boot(tester);
      app.fake.notificationOps.add(_op(NotificationOpKind.schedule, at: _at));
      await settle(tester);
      app.fake.notificationOps.add(_op(NotificationOpKind.cancel, id: 9));
      await settle(tester);
      expect(
        app.platform.calls.where(
          (c) => c.method != 'initialize' && c.method != 'launchTap',
        ),
        [
          _scheduled(_at),
          const PlatformCall('cancel', {'id': 9}),
        ],
      );
      expect(
        app.fake.calls.where((c) => c.method == 'reportNotificationResult'),
        [
          const CoreCall('reportNotificationResult', {
            'id': 4211,
            'result': NotificationResult.ok,
          }),
          const CoreCall('reportNotificationResult', {
            'id': 9,
            'result': NotificationResult.ok,
          }),
        ],
      );
    });

    testWidgets('a notification tap deep-links to the task', (tester) async {
      final app = await boot(tester, size: StrataTestSizes.expanded);
      app.platform.onTap!(
        const NotificationTap(id: 4211, payload: 't-petrol-arrows'),
      );
      await settle(tester);
      expect(app.router.state.uri.path, '/tasks/t-petrol-arrows');
    });

    testWidgets('an action tap in the foreground is forwarded', (tester) async {
      final app = await boot(tester);
      app.platform.onTap!(
        const NotificationTap(id: 4211, actionId: ReminderActions.snooze),
      );
      await settle(tester);
      expect(app.fake.calls.last.method, 'notificationAction');
      expect(app.router.state.uri.path, '/home');
    });

    testWidgets('the tap that launched the app opens its task', (tester) async {
      final app = await boot(
        tester,
        platform: FakeNotificationPlatform(
          launch: const NotificationTap(id: 4211, payload: 't-petrol-arrows'),
        ),
      );
      expect(app.router.state.uri.path, '/tasks/t-petrol-arrows');
      expect(find.byType(NavigationBar), findsNothing);
    });

    testWidgets('no adapter while signed out', (tester) async {
      final app = await boot(tester, session: StrataFixtures.sessionSignedOut);
      expect(app.platform.calls, isEmpty);
      expect(
        app.fake.calls.where((c) => c.method == 'watchNotificationOps'),
        isEmpty,
      );
    });
  });

  test('other action identifiers are not forwarded', () {
    expect(actionOf(const NotificationTap(id: 1, actionId: 'other')), isNull);
  });
}
