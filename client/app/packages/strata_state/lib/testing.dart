/// Test support shared by every feature package: [FakeCoreApi] (a
/// controllable [CoreApi]), [pumpStrataScreen] / [StrataTestFrame] (a screen
/// inside ProviderScope + MaterialApp with the Strata theme and
/// localisations), [StrataFixtures] (sample view-models from
/// `design/SCREEN_SPEC.md`), and the PLAN §16.5 matrix: [Variant],
/// [variants] / [goldenVariants], [pumpVariant], [expectAccessible],
/// [goldenFrame] / [screenGoldens] and [strataTestExecutable] (the
/// `flutter_test_config.dart` of every package).
///
/// Import only from tests:
///
/// ```dart
/// testWidgets('shows the recent notes', (tester) async {
///   final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
///   await pumpStrataScreen(tester, const HomeScreen(), fake: fake);
///   expect(find.text('Pricing experiments'), findsOneWidget);
///   await tester.tap(find.byTooltip('Capture'));
///   expect(fake.calls.last, const CoreCall('capture', {'text': '…'}));
/// });
/// ```
library;

import 'package:strata_state/src/core_api.dart';
import 'package:strata_state/src/testing/fake_core_api.dart';
import 'package:strata_state/src/testing/fixtures.dart';
import 'package:strata_state/src/testing/matrix.dart';
import 'package:strata_state/src/testing/pump.dart';

export 'src/testing/fake_core_api.dart';
export 'src/testing/fixtures.dart';
export 'src/testing/matrix.dart';
export 'src/testing/pump.dart';
