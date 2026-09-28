import 'dart:io' as io;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata_bridge/strata_bridge.dart' as bridge;
import 'package:strata_state/strata_state.dart';

part 'core_bootstrap.g.dart';

/// The platform glue that has to run before the Rust core can open: loading
/// the native library and describing this install (PLAN §12). Plumbing only;
/// the core decides everything else. Tests replace it.
abstract interface class CoreBootstrap {
  /// Loads the native core library.
  Future<void> loadLibrary();

  /// This install's configuration for `initCore`.
  Future<CoreConfig> config();
}

/// The production bootstrap: `loadStrataCore`, the app-support directory
/// from `strataAppDataDirectory`, the platform, the host name as the default
/// device name and the build's default server address.
class NativeCoreBootstrap implements CoreBootstrap {
  /// Creates the bootstrap.
  const new();

  @override
  Future<void> loadLibrary() => bridge.loadStrataCore();

  @override
  Future<CoreConfig> config() async => CoreConfig(
    appDataDir: await bridge.strataAppDataDirectory(),
    platform: platformOf(defaultTargetPlatform),
    defaultDeviceName: io.Platform.localHostname,
    defaultServerUrl: defaultServer,
  );

  /// The server address baked in at build time
  /// (`--dart-define=STRATA_DEFAULT_SERVER=<url>`, docs/RUNBOOK.md §13);
  /// empty when unset, which the core treats as no default.
  static const String defaultServer = String.fromEnvironment(
    'STRATA_DEFAULT_SERVER',
  );
}

/// The core's [Platform] for a Flutter [TargetPlatform] (1:1; Fuchsia is
/// not a target and reports as Linux).
Platform platformOf(TargetPlatform target) => switch (target) {
  TargetPlatform.android => Platform.android,
  TargetPlatform.iOS => Platform.ios,
  TargetPlatform.macOS => Platform.macos,
  TargetPlatform.windows => Platform.windows,
  TargetPlatform.linux || TargetPlatform.fuchsia => Platform.linux,
};

/// The bootstrap in use (overridden in tests).
@Riverpod(keepAlive: true)
CoreBootstrap coreBootstrap(Ref ref) => const NativeCoreBootstrap();

/// Starts the core once: loads the library, then `initCore` with this
/// install's configuration. The result is the session to route on until the
/// session stream emits (the app shows a splash meanwhile).
@Riverpod(keepAlive: true, retry: noCoreRetry)
Future<SessionState> coreStartup(Ref ref) async {
  final bootstrap = ref.watch(coreBootstrapProvider);
  await bootstrap.loadLibrary();
  final config = await bootstrap.config();
  return await ref.watch(coreApiProvider).initCore(config: config);
}
