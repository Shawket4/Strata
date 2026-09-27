import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:hooks_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

/// One cell of the §16.5 matrix: window size × theme × UI language × text
/// scale.
@immutable
class Variant {
  /// Creates a variant.
  const Variant(
    this.sizeName,
    this.size,
    this.brightness,
    this.locale,
    this.textScale,
  );

  /// `compact`, `medium`, `expanded` or `large`.
  final String sizeName;

  /// Window size.
  final Size size;

  /// Theme.
  final Brightness brightness;

  /// UI language.
  final Locale locale;

  /// Text scale.
  final double textScale;

  /// The size class of [size].
  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  /// Whether the UI is right-to-left.
  bool get rtl => locale.languageCode == 'ar';

  /// A file-name-safe ID.
  String get id =>
      '${sizeName}_${brightness.name}_${rtl ? 'rtl' : 'ltr'}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

const _locales = [StrataLocales.english, StrataLocales.arabic];

/// Every size × light/dark × en/ar × 1.0/2.0 (32 variants).
List<Variant> matrix({Map<String, Size> sizes = StrataTestSizes.all}) => [
  for (final size in sizes.entries)
    for (final brightness in Brightness.values)
      for (final locale in _locales)
        for (final scale in const [1.0, 2.0])
          Variant(size.key, size.value, brightness, locale, scale),
];

/// The golden set: compact / medium / expanded × light/dark × LTR/RTL at
/// 1.0, plus compact at 2.0 (16 variants).
List<Variant> goldenMatrix() => [
  for (final size in const {
    'compact': StrataTestSizes.compact,
    'medium': StrataTestSizes.medium,
    'expanded': StrataTestSizes.expanded,
  }.entries)
    for (final brightness in Brightness.values)
      for (final locale in _locales)
        for (final scale in size.key == 'compact' ? const [1.0, 2.0] : [1.0])
          Variant(size.key, size.value, brightness, locale, scale),
];

/// Pumps [screen] for [v] with [fake] and lets replayed stream values land.
Future<FakeCoreApi> pumpVariant(
  WidgetTester tester,
  Variant v,
  Widget screen, {
  FakeCoreApi? fake,
  List<Override> overrides = const [],
}) async {
  final api = await pumpStrataScreen(
    tester,
    screen,
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

/// Pumps a few frames (spinners never settle).
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Tap-target, labelled-tap-target and text-contrast guidelines.
Future<void> expectAccessible(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
  await expectLater(tester, meetsGuideline(textContrastGuideline));
  handle.dispose();
}

/// No exception (e.g. an overflow) was reported.
void expectNoErrors(WidgetTester tester) =>
    expect(tester.takeException(), isNull);

/// The UI direction of the frame.
TextDirection directionOf(WidgetTester tester, Finder finder) =>
    Directionality.of(tester.element(finder.first));

/// A golden frame: [screen] in the Strata test frame for [v], at the
/// variant's window size.
Widget goldenFrame(
  Variant v,
  Widget screen, {
  FakeCoreApi? fake,
  List<Override> overrides = const [],
}) => Builder(
  builder: (context) => MediaQuery(
    data: MediaQuery.of(context).copyWith(size: v.size),
    child: SizedBox(
      width: v.size.width,
      height: v.size.height,
      child: StrataTestFrame(
        api: fake ?? FakeCoreApi(),
        brightness: v.brightness,
        locale: v.locale,
        textScale: v.textScale,
        overrides: overrides,
        child: screen,
      ),
    ),
  ),
);

/// Registers one alchemist golden per [goldenMatrix] variant.
void goldens(
  String name,
  Widget Function(Variant v) builder, {
  PumpAction? pump,
}) {
  for (final v in goldenMatrix()) {
    goldenTest(
      '$name ${v.id}',
      fileName: '${name}_${v.id}',
      pumpBeforeTest: pump ?? pumpNTimes(4, const Duration(milliseconds: 100)),
      builder: () => builder(v),
    );
  }
}
