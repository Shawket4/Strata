import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:strata/strata.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// A [CoreBootstrap] that loads nothing and reports the fixture config;
/// [gate] holds the start until completed (splash tests).
class FakeBootstrap implements CoreBootstrap {
  /// Creates the bootstrap.
  new({this.gate, this.error});

  /// Completes the library load when given.
  final Completer<void>? gate;

  /// Thrown by the load when set.
  Exception? error;

  /// How often the library was loaded.
  int loads = 0;

  @override
  Future<void> loadLibrary() async {
    loads++;
    await gate?.future;
    final failure = error;
    if (failure != null) throw failure;
  }

  @override
  Future<CoreConfig> config() async => StrataFixtures.coreConfig;
}

/// One call on the [FakeNotificationPlatform].
@immutable
class PlatformCall {
  /// Creates the record.
  const new(this.method, [this.args = const {}]);

  /// `initialize`, `schedule`, `show`, `cancel`, `launchTap`.
  final String method;

  /// Arguments.
  final Map<String, Object?> args;

  @override
  bool operator ==(Object other) =>
      other is PlatformCall &&
      other.method == method &&
      mapEquals(other.args, args);

  @override
  int get hashCode => Object.hash(method, Object.hashAllUnordered(args.keys));

  @override
  String toString() => '$method($args)';
}

/// A [NotificationPlatform] that records every call and answers with
/// [result].
class FakeNotificationPlatform implements NotificationPlatform {
  /// Creates the fake.
  new({this.launch});

  /// Every call, in order.
  final List<PlatformCall> calls = [];

  /// The answer to schedule / show / cancel.
  NotificationResult result = NotificationResult.ok;

  /// The tap that "launched" the app.
  final NotificationTap? launch;

  /// The tap handler the adapter registered.
  ValueChanged<NotificationTap>? onTap;

  /// The strings the adapter passed.
  NotificationStrings? strings;

  @override
  Future<void> initialize({
    required NotificationStrings strings,
    required ValueChanged<NotificationTap> onTap,
  }) async {
    calls.add(const PlatformCall('initialize'));
    this.strings = strings;
    this.onTap = onTap;
  }

  @override
  Future<NotificationTap?> launchTap() async {
    calls.add(const PlatformCall('launchTap'));
    return launch;
  }

  @override
  Future<NotificationResult> schedule({
    required int id,
    required DateTime at,
    required String title,
    required String body,
    required String payload,
  }) async {
    calls.add(
      PlatformCall('schedule', {
        'id': id,
        'at': at,
        'title': title,
        'body': body,
        'payload': payload,
      }),
    );
    return result;
  }

  @override
  Future<NotificationResult> show({
    required int id,
    required String title,
    required String body,
    required String payload,
  }) async {
    calls.add(
      PlatformCall('show', {
        'id': id,
        'title': title,
        'body': body,
        'payload': payload,
      }),
    );
    return result;
  }

  @override
  Future<NotificationResult> cancel({required int id}) async {
    calls.add(PlatformCall('cancel', {'id': id}));
    return result;
  }
}
