import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// Alchemist: only the CI variant, rendered with the real bundled fonts
/// (goldens load them in `setUpAll(loadStrataFonts)`); widget tests keep
/// Flutter's test font, which the text-contrast guideline measures exactly.
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  TestWidgetsFlutterBinding.ensureInitialized();
  await AlchemistConfig.runWithConfig<FutureOr<void>>(
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
