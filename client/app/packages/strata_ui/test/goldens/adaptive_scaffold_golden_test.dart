import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/harness.dart';

/// The adaptive shell with list / detail / context panes at compact (390 ×
/// 844), medium (1024 × 768, context drawer open), expanded (1440 × 900) and
/// large (1920 × 1080) × light/dark × LTR/RTL × text scale 1.0/2.0.
void main() {
  setUpAll(loadStrataFonts);

  group('adaptive scaffold goldens', () {
    for (final v in variants()) {
      unawaited(
        goldenTest(
          'adaptive scaffold $v',
          fileName: 'adaptive_scaffold_${v.id}',
          builder: () => goldenFrame(
            v,
            TestShell(
              body: SamplePanes(contextOpen: v.sizeClass == SizeClass.medium),
            ),
          ),
        ),
      );
    }
  });
}
