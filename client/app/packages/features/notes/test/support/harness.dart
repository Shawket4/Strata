// The §16.5 test matrix for this package's screens: window size × theme ×
// UI language/direction × text scale, a widget-test pump and an alchemist
// golden frame around a FakeCoreApi.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// One cell of the matrix.
@immutable
class MatrixVariant {
  const new({
    required this.sizeName,
    required this.size,
    required this.brightness,
    required this.locale,
    required this.textScale,
  });

  final String sizeName;
  final Size size;
  final Brightness brightness;
  final Locale locale;
  final double textScale;

  bool get rtl => locale.languageCode == 'ar';

  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  ThemeData get theme => brightness == Brightness.dark
      ? StrataTheme.dark(platform: TargetPlatform.android)
      : StrataTheme.light(platform: TargetPlatform.android);

  String get id =>
      '${sizeName}_${brightness.name}_${rtl ? 'rtl' : 'ltr'}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

const List<Locale> _locales = [StrataLocales.english, StrataLocales.arabic];

/// Every combination of [sizes] × light/dark × en/ar × [textScales].
List<MatrixVariant> variants({
  Map<String, Size> sizes = StrataTestSizes.all,
  List<double> textScales = const [1, 2],
}) => [
  for (final size in sizes.entries)
    for (final brightness in Brightness.values)
      for (final locale in _locales)
        for (final scale in textScales)
          MatrixVariant(
            sizeName: size.key,
            size: size.value,
            brightness: brightness,
            locale: locale,
            textScale: scale,
          ),
];

/// The golden matrix: every size class × theme × direction at 1.0, plus
/// compact at 2.0.
List<MatrixVariant> goldenVariants() => [
  for (final v in variants(
    sizes: const {
      'compact': StrataTestSizes.compact,
      'medium': StrataTestSizes.medium,
      'expanded': StrataTestSizes.expanded,
    },
  ))
    if (v.textScale == 1 || v.sizeClass == SizeClass.compact) v,
];

/// Pumps [screen] for [v] with [fake] as the core.
Future<void> pumpVariant(
  WidgetTester tester,
  MatrixVariant v,
  Widget screen,
  FakeCoreApi fake,
) async {
  await pumpStrataScreen(
    tester,
    screen,
    fake: fake,
    size: v.size,
    theme: v.brightness,
    locale: v.locale,
    textScale: v.textScale,
  );
  await tester.pump();
}

/// Wraps [child] for an alchemist golden of [v]: [fake] as the core, the
/// Strata theme, localisations, window size and text scale.
Widget goldenFrame(MatrixVariant v, FakeCoreApi fake, Widget child) {
  final container = ProviderContainer(
    overrides: [coreApiProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  return UncontrolledProviderScope(
    container: container,
    child: Localizations(
      locale: v.locale,
      delegates: StrataLocalizations.localizationsDelegates,
      child: Theme(
        data: v.theme,
        child: Builder(
          builder: (context) => MediaQuery(
            data: MediaQuery.of(context).copyWith(
              size: v.size,
              textScaler: TextScaler.linear(v.textScale),
              platformBrightness: v.brightness,
            ),
            child: SizedBox(
              width: v.size.width,
              height: v.size.height,
              child: ColoredBox(
                color: context.strataColors.background,
                child: child,
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

/// Runs the tap-target, labelled-tap-target and text-contrast guidelines.
Future<void> expectAccessible(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
  await expectLater(tester, meetsGuideline(textContrastGuideline));
  handle.dispose();
}

/// Asserts that no exception (e.g. a RenderFlex overflow) was reported.
void expectNoRenderErrors(WidgetTester tester) {
  expect(tester.takeException(), isNull);
}
