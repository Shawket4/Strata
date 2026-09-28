import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/strata.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';


void main() {
  setUpAll(loadStrataFonts);

  screenGoldens(
    'splash',
    (v) => goldenFrame(v, const SplashScreen()),
    pump: pumpNTimes(2, const Duration(milliseconds: 100)),
  );
  screenGoldens(
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
