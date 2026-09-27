import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// One cell of the §16.5 matrix: window size × theme × UI language ×
/// text scale.
@immutable
class Variant {
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

  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  bool get rtl => locale.languageCode == 'ar';

  String get id =>
      '${sizeName}_${brightness.name}_${rtl ? 'rtl' : 'ltr'}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

/// Every combination of [sizes] × light/dark × en/ar × [scales].
List<Variant> matrix({
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

/// The golden subset: compact / medium / expanded × light/dark × LTR/RTL at
/// 1.0, plus compact at 2.0.
List<Variant> goldenMatrix({
  Map<String, Size> sizes = const {
    'compact': StrataTestSizes.compact,
    'medium': StrataTestSizes.medium,
    'expanded': StrataTestSizes.expanded,
  },
}) => [
  for (final v in matrix(sizes: sizes))
    if (v.textScale == 1 || v.sizeName == 'compact') v,
];

/// Pumps [screen] (inside a Scaffold, as the app shell hosts it) for [v].
Future<FakeCoreApi> pumpVariant(
  WidgetTester tester,
  Variant v,
  Widget screen, {
  FakeCoreApi? fake,
  bool settle = true,
  bool scaffold = true,
}) async {
  final api = await pumpStrataScreen(
    tester,
    scaffold ? Scaffold(body: screen) : screen,
    fake: fake,
    size: v.size,
    theme: v.brightness,
    locale: v.locale,
    textScale: v.textScale,
  );
  if (settle) {
    await tester.pumpAndSettle();
  } else {
    await tester.pump();
  }
  return api;
}

/// Tap-target, labelled-tap-target and text-contrast guidelines.
Future<void> expectAccessible(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
  await expectLater(tester, meetsGuideline(textContrastGuideline));
  handle.dispose();
}

/// No exception (e.g. a RenderFlex overflow) was reported.
void expectNoErrors(WidgetTester tester) =>
    expect(tester.takeException(), isNull);

/// [screen] framed for an alchemist golden at [v] (the window size reaches
/// `SizeClass.of` through the MediaQuery).
Widget goldenScreen(
  Variant v,
  Widget screen,
  FakeCoreApi fake, {
  bool scaffold = true,
}) {
  return MediaQuery(
    data: MediaQueryData(size: v.size),
    child: SizedBox.fromSize(
      size: v.size,
      child: StrataTestFrame(
        api: fake,
        brightness: v.brightness,
        locale: v.locale,
        textScale: v.textScale,
        child: scaffold ? Scaffold(body: screen) : screen,
      ),
    ),
  );
}

/// The shared strings for [v]'s locale.
StrataLocalizations lookupStrataLocalizationsFor(Variant v) =>
    lookupStrataLocalizations(v.locale);
