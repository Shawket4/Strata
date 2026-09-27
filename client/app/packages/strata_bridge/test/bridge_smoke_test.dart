// Loads the real Rust core through the generated bindings on the Linux host
// (no Flutter engine needed: flutter_rust_bridge talks to the library over
// dart:ffi). Build it first: `cargo build -p strata-core` (debug) from the
// repository root.
@TestOn('linux')
library;

import 'dart:io' as io;

import 'package:flutter_test/flutter_test.dart';
import 'package:strata_bridge/strata_bridge.dart';

/// `target/debug/libstrata_core.so` of the repository's Cargo workspace.
final io.File _library = io.File(
  '${io.Directory.current.path}/../../../../target/debug/libstrata_core.so',
);

void main() {
  setUpAll(() async {
    if (_library.existsSync()) {
      await loadStrataCore(library: ExternalLibrary.open(_library.path));
    }
  });

  test(
    'a fresh install routes to sign-in with the configured defaults',
    () async {
      final dir = io.Directory.systemTemp.createTempSync('strata_bridge');
      addTearDown(() => dir.deleteSync(recursive: true));

      final state = await initCore(
        config: CoreConfig(
          appDataDir: dir.path,
          platform: Platform.linux,
          defaultDeviceName: 'Linux desktop',
          defaultServerUrl: 'https://strata.example',
        ),
      );

      expect(state.kind, SessionKind.signedOut);
      expect(state.account, isNull);
      expect(state.knownAccounts, isEmpty);
      expect(state.serverUrl, 'https://strata.example');
      expect(state.deviceName, 'Linux desktop');
      expect(state.unsyncedOps, 0);
      expect(
        io.File('${dir.path}/strata/registry.sqlite3').existsSync(),
        isTrue,
      );
    },
    skip: _library.existsSync()
        ? false
        : 'build the core first: cargo build -p strata-core',
  );

  test(
    'errors cross the bridge as typed CoreFailure values',
    () async {
      final dir = io.Directory.systemTemp.createTempSync('strata_bridge');
      addTearDown(() => dir.deleteSync(recursive: true));
      await initCore(
        config: CoreConfig(
          appDataDir: dir.path,
          platform: Platform.linux,
          defaultDeviceName: 'Linux desktop',
        ),
      );

      await expectLater(
        capture(text: 'Idea: loyalty tier'),
        throwsA(
          isA<CoreFailure>()
              .having((f) => f.code, 'code', 'not_signed_in')
              .having((f) => f.messageKey, 'messageKey', 'error.not_signed_in'),
        ),
      );
    },
    skip: _library.existsSync()
        ? false
        : 'build the core first: cargo build -p strata-core',
  );
}
