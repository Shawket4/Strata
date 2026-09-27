import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/strata.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/matrix.dart';

void main() {
  setUpAll(loadStrataFonts);

  goldens(
    'splash',
    (v) => goldenFrame(v, const SplashScreen()),
    pump: pumpNTimes(2, const Duration(milliseconds: 100)),
  );
  goldens(
    'boot_failed',
    (v) => goldenFrame(
      v,
      BootFailedScreen(
        error: const CoreFailure(code: 'storage', messageKey: 'error.storage'),
        onRetry: () {},
      ),
    ),
  );
}
