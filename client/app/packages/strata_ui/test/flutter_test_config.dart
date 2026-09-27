import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// Configures alchemist: only the CI golden variant is generated, rendered
/// with the real bundled fonts (not obscured) so goldens show Cairo / Plex
/// Mono / Quicksand on the pinned CI image. Golden files load the fonts in
/// `setUpAll(loadStrataFonts)`; widget tests keep Flutter's solid test font,
/// which the text-contrast guideline needs to measure glyph colours exactly.
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  TestWidgetsFlutterBinding.ensureInitialized();
  return AlchemistConfig.runWithConfig(
    config: AlchemistConfig(
      platformGoldensConfig: const PlatformGoldensConfig(enabled: false),
      ciGoldensConfig: const CiGoldensConfig(obscureText: false),
      goldenTestTheme: GoldenTestTheme(
        backgroundColor: const Color(0xFFFFFFFF),
        borderColor: const Color(0x00000000),
        nameTextStyle: const TextStyle(fontSize: 12),
      ),
    ),
    run: testMain,
  );
}
