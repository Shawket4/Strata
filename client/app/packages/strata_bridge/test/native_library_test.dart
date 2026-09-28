// Reads the podspecs of this package: the test runs with the package root as
// its working directory.
import 'dart:io' as io;

import 'package:flutter_test/flutter_test.dart';
import 'package:strata_bridge/strata_bridge.dart';

/// On macOS and iOS the core is a static library force-loaded into this
/// plugin's pod, so it lives in the pod's framework, not in one named after
/// the crate (flutter_rust_bridge's default lookup). Every such build failed
/// at start with "Strata couldn't start (internal)".
void main() {
  for (final os in ['macos', 'ios']) {
    test('$os loads the core from the pod framework', () {
      final podspec = io.File('$os/strata_bridge.podspec').readAsStringSync();
      final name = RegExp(r"s\.name\s*=\s*'([^']+)'")
          .firstMatch(podspec)!
          .group(1);
      expect(podspec, contains('-force_load'));
      expect(appleCoreFramework(os), '$name.framework/$name');
    });
  }

  test('other platforms use the default lookup', () {
    for (final os in ['android', 'linux', 'windows']) {
      expect(appleCoreFramework(os), isNull, reason: os);
    }
  });
}
