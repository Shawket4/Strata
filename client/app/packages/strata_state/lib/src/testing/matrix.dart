import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/src/testing/fake_core_api.dart';
import 'package:strata_state/src/testing/pump.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// One cell of the PLAN §16.5 matrix: window size × theme × UI language ×
/// text scale.
@immutable
class Variant {
  /// Creates a variant.
  const new({
    required this.sizeName,
    required this.size,
    required this.brightness,
    required this.locale,
    required this.textScale,
  });

  /// `compact`, `medium`, `expanded` or `large`.
  final String sizeName;

  /// Window size.
  final Size size;

  /// Theme.
  final Brightness brightness;

  /// UI language (the direction follows it).
  final Locale locale;

  /// Text scale.
  final double textScale;

  /// The size class of [size].
  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  /// Whether the UI is right-to-left.
  bool get rtl => locale.languageCode == 'ar';

  /// The shared strings of [locale].
  StrataLocalizations get l10n => lookupStrataLocalizations(locale);

  /// A file-name-safe ID (`compact_light_ltr_1x`).
  String get id =>
      '${sizeName}_${brightness.name}_${rtl ? 'rtl' : 'ltr'}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

/// The golden window sizes: compact, medium and expanded.
const Map<String, Size> goldenSizes = {
  'compact': StrataTestSizes.compact,
  'medium': StrataTestSizes.medium,
  'expanded': StrataTestSizes.expanded,
};

/// Every [sizes] × light/dark × en/ar × [scales] (32 variants by default).
List<Variant> variants({
  Map<String, Size> sizes = StrataTestSizes.all,
  List<double> scales = const [1, 2],
}) => [
  for (final size in sizes.entries)
    for (final brightness in Brightness.values)
      for (final locale in const [StrataLocales.english, StrataLocales.arabic])
        for (final scale in scales)
          Variant(
            sizeName: size.key,
            size: size.value,
            brightness: brightness,
            locale: locale,
            textScale: scale,
          ),
];

/// The golden set: [sizes] (compact / medium / expanded) × light/dark ×
/// LTR/RTL at 1.0, plus compact at 2.0 (16 variants). [compact] / [wide]
/// drop the compact or the medium and expanded sizes.
List<Variant> goldenVariants({
  Map<String, Size> sizes = goldenSizes,
  bool compact = true,
  bool wide = true,
}) => [
  for (final v in variants(sizes: sizes))
    if ((v.sizeClass == SizeClass.compact ? compact : wide) &&
        (v.textScale == 1 || v.sizeClass == SizeClass.compact))
      v,
];

/// Pumps a few frames, 100 ms apart (spinners never settle).
Future<void> settle(WidgetTester tester, {int frames = 10}) async {
  for (var i = 0; i < frames; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Pumps [screen] for [v] (inside a [Scaffold] when [scaffold], as the app
/// shell hosts a destination) with [fake] as the core, lets replayed stream
/// values land ([settle]) and returns the fake.
Future<FakeCoreApi> pumpVariant(
  WidgetTester tester,
  Variant v,
  Widget screen, {
  FakeCoreApi? fake,
  List<Override> overrides = const [],
  bool scaffold = false,
}) async {
  final api = await pumpStrataScreen(
    tester,
    scaffold ? Scaffold(body: screen) : screen,
    fake: fake,
    size: v.size,
    theme: v.brightness,
    locale: v.locale,
    textScale: v.textScale,
    overrides: overrides,
  );
  await settle(tester);
  return api;
}

/// The tap-target, labelled-tap-target and text-contrast guidelines.
///
/// [contrast] can be turned off where a scroll view clips a line at its
/// edge at 2x text: the guideline samples a text's whole paint bounds, so
/// the clipped part reads the pixels outside the viewport (the colours are
/// the same as at 1x, where contrast is checked).
Future<void> expectAccessible(
  WidgetTester tester, {
  bool contrast = true,
}) async {
  final handle = tester.ensureSemantics();
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
  if (contrast) {
    await expectLater(tester, meetsGuideline(textContrastGuideline));
  }
  handle.dispose();
}

/// No exception (e.g. a RenderFlex overflow) was reported.
void expectNoErrors(WidgetTester tester) =>
    expect(tester.takeException(), isNull);

/// The UI direction at [finder].
TextDirection directionOf(WidgetTester tester, Finder finder) =>
    Directionality.of(tester.element(finder.first));

/// Scrolls [finder] into view, then taps it and lets the result land.
Future<void> tapVisible(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await settle(tester);
  await tester.tap(finder);
  await settle(tester);
}

/// A golden frame: [screen] (inside a [Scaffold] when [scaffold]) in the
/// Strata test frame for [v], at the variant's window size (which reaches
/// `SizeClass.of` through the MediaQuery).
Widget goldenFrame(
  Variant v,
  Widget screen, {
  FakeCoreApi? fake,
  List<Override> overrides = const [],
  bool scaffold = false,
}) => SizedBox.fromSize(
  size: v.size,
  child: StrataTestFrame(
    api: fake ?? FakeCoreApi(),
    brightness: v.brightness,
    locale: v.locale,
    textScale: v.textScale,
    overrides: overrides,
    child: Builder(
      builder: (context) => MediaQuery(
        data: MediaQuery.of(context).copyWith(size: v.size),
        child: scaffold ? Scaffold(body: screen) : screen,
      ),
    ),
  ),
);

/// Registers one alchemist golden `<name>_<variant id>` per [cells]
/// (default [goldenVariants]); [builder] returns the framed widget (usually
/// a [goldenFrame]).
void screenGoldens(
  String name,
  Widget Function(Variant v) builder, {
  List<Variant>? cells,
  PumpAction? pump,
  Interaction? whilePerforming,
}) {
  for (final v in cells ?? goldenVariants()) {
    unawaited(
      goldenTest(
        '$name ${v.id}',
        fileName: '${name}_${v.id}',
        constraints: BoxConstraints.tight(v.size),
        pumpBeforeTest:
            pump ?? pumpNTimes(4, const Duration(milliseconds: 100)),
        whilePerforming: whilePerforming,
        builder: () => builder(v),
      ),
    );
  }
}

/// The `testExecutable` of every package's `flutter_test_config.dart`.
///
/// Alchemist renders only the CI variant, with the real bundled fonts (not
/// obscured) so goldens show Cairo / Plex Mono / Quicksand on the pinned CI
/// image; golden files load the fonts in `setUpAll(loadStrataFonts)`.
/// Widget tests keep Flutter's solid test font, which the text-contrast
/// guideline needs to measure glyph colours exactly.
Future<void> strataTestExecutable(FutureOr<void> Function() testMain) async {
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
