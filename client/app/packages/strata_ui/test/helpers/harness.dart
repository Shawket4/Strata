import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// One cell of the §16.5 test matrix: window size × brightness × UI
/// language/direction × text scale.
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

  TextDirection get direction =>
      locale.languageCode == 'ar' ? TextDirection.rtl : TextDirection.ltr;

  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  ThemeData get theme => brightness == Brightness.dark
      ? StrataTheme.dark(platform: TargetPlatform.android)
      : StrataTheme.light(platform: TargetPlatform.android);

  String get id =>
      '${sizeName}_${brightness.name}_${direction.name}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

const List<Locale> _locales = [StrataLocales.english, StrataLocales.arabic];

/// Every combination of [sizes] × light/dark × LTR/RTL × text scale 1.0/2.0.
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

/// Sets the test window to the variant size at a device pixel ratio of 1.
void setWindowSize(WidgetTester tester, Size size) {
  tester.view
    ..physicalSize = size
    ..devicePixelRatio = 1;
  addTearDown(tester.view.reset);
}

/// Pumps [child] as the home of a [MaterialApp] configured for [v].
Future<void> pumpVariant(
  WidgetTester tester,
  MatrixVariant v,
  Widget child, {
  bool settle = true,
}) async {
  setWindowSize(tester, v.size);
  await tester.pumpWidget(
    MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: v.theme,
      locale: v.locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      builder: (context, app) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(v.textScale)),
        child: app!,
      ),
      home: child,
    ),
  );
  if (settle) await tester.pumpAndSettle();
}

/// Wraps [child] in theme, localizations and media query for [v], sized to
/// [frame] (for alchemist goldens, which render inside their own app). With
/// [intrinsicHeight] the frame keeps [frame]'s width and takes the child's
/// natural height.
Widget goldenFrame(
  MatrixVariant v,
  Widget child, {
  Size? frame,
  bool intrinsicHeight = false,
}) {
  final size = frame ?? v.size;
  return Localizations(
    locale: v.locale,
    delegates: StrataLocalizations.localizationsDelegates,
    child: Theme(
      data: v.theme,
      child: Builder(
        builder: (context) => MediaQuery(
          data: MediaQuery.of(context).copyWith(
            size: size,
            textScaler: TextScaler.linear(v.textScale),
            platformBrightness: v.brightness,
          ),
          child: SizedBox(
            width: size.width,
            height: intrinsicHeight ? null : size.height,
            child: ColoredBox(
              color: context.strataColors.background,
              child: child,
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

/// The semantics data of the node found by [finder].
SemanticsData semanticsOf(WidgetTester tester, Finder finder) =>
    tester.getSemantics(finder).getSemanticsData();
