import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/l10n.dart';
import 'package:strata/src/reminders/local_notifications_platform.dart';
import 'package:strata/src/reminders/notification_platform.dart';
import 'package:strata_state/strata_state.dart';

/// Snooze length sent with a Snooze action until the core provides one
/// (docs/CORE_GAPS.md "Snooze length"). The copy in the design says 15 min.
const int interimSnoozeMinutes = 15;

/// The core's action for a tapped notification action, or `null` for a tap
/// on the notification itself (1:1 on the action identifier).
NotificationAction? actionOf(NotificationTap tap) => switch (tap.actionId) {
  ReminderActions.done => const NotificationAction(
    kind: NotificationActionKind.done,
    minutes: 0,
  ),
  ReminderActions.snooze => const NotificationAction(
    kind: NotificationActionKind.snooze,
    minutes: interimSnoozeMinutes,
  ),
  _ => null,
};

/// Relays the core's notification ops to the platform and the platform's
/// answers back (PLAN §12.5b, "Dart only relays"): no filtering, sorting or
/// time maths.
class ReminderAdapter {
  /// Creates the adapter.
  new({required this.core, required this.platform, required this.onOpenTask});

  /// The Rust core.
  final CoreApi core;

  /// The notification plugin seam.
  final NotificationPlatform platform;

  /// Deep link to a task (a tap on the notification itself).
  final ValueChanged<String> onOpenTask;

  Future<void> _queue = Future.value();

  /// Applies [op] after every earlier op (the core's order matters, e.g. a
  /// cancel before a schedule of the same ID) and reports the result.
  Future<void> enqueue(NotificationOp op) =>
      _queue = _queue.then((_) => apply(op));

  /// Applies one op and reports the platform's result to the core.
  Future<void> apply(NotificationOp op) async {
    final at = op.at;
    final result = switch (op.kind) {
      NotificationOpKind.schedule ||
      NotificationOpKind.update when at != null => await platform.schedule(
        id: op.id,
        at: at,
        title: op.title,
        body: op.body,
        payload: op.taskId,
      ),
      // A schedule without an instant (not sent by the core) and show-now
      // both display immediately.
      NotificationOpKind.schedule ||
      NotificationOpKind.update ||
      NotificationOpKind.showNow => await platform.show(
        id: op.id,
        title: op.title,
        body: op.body,
        payload: op.taskId,
      ),
      NotificationOpKind.cancel => await platform.cancel(id: op.id),
    };
    await core.reportNotificationResult(id: op.id, result: result);
  }

  /// Handles a tap: Done/Snooze go to `notificationAction`; a tap on the
  /// notification opens its task.
  Future<void> handleTap(NotificationTap tap) async {
    final action = actionOf(tap);
    if (action != null) {
      await core.notificationAction(id: tap.id, action: action);
      return;
    }
    final task = tap.payload;
    if (task != null && task.isNotEmpty) onOpenTask(task);
  }
}

/// The notification plugin seam in use (a fake in tests).
final notificationPlatformProvider = Provider<NotificationPlatform>(
  (ref) => LocalNotificationsPlatform(),
);

/// Runs the [ReminderAdapter] while a session is active: initialises the
/// platform once, listens to `notificationOpsProvider` and forwards taps.
class ReminderAdapterHost extends ConsumerStatefulWidget {
  /// Creates the host around [child].
  const new({required this.child, required this.onOpenTask, super.key});

  /// The shell.
  final Widget child;

  /// Deep link to a task.
  final ValueChanged<String> onOpenTask;

  @override
  ConsumerState<ReminderAdapterHost> createState() =>
      _ReminderAdapterHostState();
}

class _ReminderAdapterHostState extends ConsumerState<ReminderAdapterHost> {
  late final ReminderAdapter _adapter = ReminderAdapter(
    core: ref.read(coreApiProvider),
    platform: ref.read(notificationPlatformProvider),
    onOpenTask: (task) => widget.onOpenTask(task),
  );
  ProviderSubscription<AsyncValue<NotificationOp>>? _ops;

  @override
  void initState() {
    super.initState();
    unawaited(_start());
  }

  Future<void> _start() async {
    final strings = appL10nFor(
      WidgetsBinding.instance.platformDispatcher.locale,
    );
    await _adapter.platform.initialize(
      strings: NotificationStrings(
        done: strings.notificationDone,
        snooze: strings.notificationSnooze,
        open: strings.notificationOpen,
        channelName: strings.channelName,
        channelDescription: strings.channelDescription,
      ),
      onTap: (tap) => unawaited(_adapter.handleTap(tap)),
    );
    if (!mounted) return;
    // A fresh subscription for this session (the stream is per session).
    ref.invalidate(notificationOpsProvider);
    _ops = ref.listenManual(notificationOpsProvider, (_, next) {
      if (next case AsyncData(:final value)) {
        unawaited(_adapter.enqueue(value));
      }
    }, fireImmediately: true);
    final launch = await _adapter.platform.launchTap();
    if (launch != null && mounted) await _adapter.handleTap(launch);
  }

  @override
  void dispose() {
    _ops?.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
