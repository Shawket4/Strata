import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/testing.dart';
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

  /// `compact` / `medium` / `expanded` / `large`.
  final String sizeName;

  /// Window size.
  final Size size;

  /// Theme.
  final Brightness brightness;

  /// UI locale.
  final Locale locale;

  /// Text scale.
  final double textScale;

  /// Size class of [size].
  SizeClass get sizeClass => SizeClass.fromWidth(size.width);

  /// Whether the UI is right-to-left.
  bool get rtl => locale == StrataLocales.arabic;

  /// File-name friendly ID.
  String get id =>
      '${sizeName}_${brightness.name}_${rtl ? 'rtl' : 'ltr'}_'
      '${textScale == 1 ? '1x' : '2x'}';

  @override
  String toString() => id;
}

/// Every size × light/dark × en/ar × 1.0/2.0.
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

/// The golden subset: compact / medium / expanded × theme × direction at
/// 1.0, plus compact at 2.0.
List<Variant> goldenVariants({bool compact = true, bool wide = true}) => [
  for (final v in variants(
    sizes: {
      if (compact) 'compact': StrataTestSizes.compact,
      if (wide) 'medium': StrataTestSizes.medium,
      if (wide) 'expanded': StrataTestSizes.expanded,
    },
  ))
    if (v.textScale == 1 || v.sizeClass == SizeClass.compact) v,
];

/// Pumps [screen] for [v] with [fake] and settles the fake's first values.
Future<void> pumpVariant(
  WidgetTester tester,
  Variant v,
  Widget screen,
  FakeCoreApi fake, {
  List<Override> overrides = const [],
}) async {
  await pumpStrataScreen(
    tester,
    Scaffold(body: screen),
    fake: fake,
    size: v.size,
    theme: v.brightness,
    locale: v.locale,
    textScale: v.textScale,
    overrides: overrides,
  );
  await tester.pump();
  await tester.pump();
}

/// Tap target, labelled tap target and text contrast guidelines.
Future<void> expectAccessible(WidgetTester tester) async {
  final handle = tester.ensureSemantics();
  await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
  await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
  await expectLater(tester, meetsGuideline(textContrastGuideline));
  handle.dispose();
}

/// No layout overflow or other exception was reported.
void expectNoErrors(WidgetTester tester) =>
    expect(tester.takeException(), isNull);

/// Registers alchemist goldens of [build] for every [goldenVariants] cell.
void screenGoldens(
  String name,
  Widget Function() build,
  FakeCoreApi Function() fake, {
  List<Variant>? cells,
  Interaction? whilePerforming,
}) {
  for (final v in cells ?? goldenVariants()) {
    unawaited(
      goldenTest(
        '$name ${v.id}',
        fileName: '${name}_${v.id}',
        tags: const ['golden'],
        constraints: BoxConstraints.tight(v.size),
        pumpBeforeTest: (tester) async {
          await tester.pump();
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 400));
        },
        whilePerforming: whilePerforming,
        builder: () => SizedBox.fromSize(
          size: v.size,
          child: StrataTestFrame(
            api: fake(),
            brightness: v.brightness,
            locale: v.locale,
            textScale: v.textScale,
            child: Builder(
              builder: (context) => MediaQuery(
                data: MediaQuery.of(context).copyWith(size: v.size),
                child: Scaffold(body: build()),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
