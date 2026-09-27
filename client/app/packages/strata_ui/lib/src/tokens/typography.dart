import 'dart:ui' show FontVariation;

import 'package:flutter/material.dart';

/// Bundled font families (OFL, see `assets/fonts/licenses`). Fonts are
/// shipped with the app and never fetched at runtime (PLAN §11).
abstract final class StrataFonts {
  /// Package that bundles the fonts.
  static const String package = 'strata_ui';

  /// Cairo (variable, Latin + Arabic): UI and all content.
  static const String cairo = 'Cairo';

  /// IBM Plex Mono (regular, medium): IDs, timestamps, markdown source,
  /// frontmatter keys, keyboard hints.
  static const String plexMono = 'IBMPlexMono';

  /// Quicksand (variable, used at 700): the "strata" wordmark only.
  static const String quicksand = 'Quicksand';

  /// Asset paths of the bundled font licences.
  static const List<String> licenseAssets = [
    'packages/strata_ui/assets/fonts/licenses/Cairo-OFL.txt',
    'packages/strata_ui/assets/fonts/licenses/IBMPlexMono-OFL.txt',
    'packages/strata_ui/assets/fonts/licenses/Quicksand-OFL.txt',
  ];
}

/// The type scale from the design spec (sizes in logical px).
abstract final class StrataTypeScale {
  /// Caption: 12.
  static const double caption = 12;

  /// Small body: 14.
  static const double bodySmall = 14;

  /// Body: 16 (15 for dense lists).
  static const double body = 16;

  /// Dense body: 15.
  static const double bodyDense = 15;

  /// Small title: 18.
  static const double titleSmall = 18;

  /// Title: 22.
  static const double title = 22;

  /// Display: 28.
  static const double display = 28;

  /// Body line height (Arabic needs 1.5).
  static const double bodyHeight = 1.5;
}

/// A [TextStyle] in a bundled family with its weight applied to both
/// [TextStyle.fontWeight] and the variable-font `wght` axis.
TextStyle strataTextStyle({
  required String family,
  required double size,
  FontWeight weight = FontWeight.w400,
  double? height,
  double? letterSpacing,
  Color? color,
}) {
  return TextStyle(
    fontFamily: family,
    package: StrataFonts.package,
    fontSize: size,
    fontWeight: weight,
    fontVariations: [FontVariation.weight(weight.value.toDouble())],
    height: height,
    letterSpacing: letterSpacing,
    color: color,
  );
}

/// Named text styles as a [ThemeExtension], coloured for one brightness.
@immutable
class StrataTextStyles extends ThemeExtension<StrataTextStyles> {
  /// Creates the named styles.
  const new({
    required this.display,
    required this.title,
    required this.titleSmall,
    required this.body,
    required this.bodyStrong,
    required this.bodySmall,
    required this.caption,
    required this.label,
    required this.mono,
    required this.monoSmall,
    required this.wordmark,
  });

  /// Builds the styles with [text] as the primary and [text2] as the
  /// secondary colour.
  factory from({required Color text, required Color text2}) {
    const cairo = StrataFonts.cairo;
    return StrataTextStyles(
      display: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.display,
        weight: FontWeight.w700,
        height: 1.25,
        color: text,
      ),
      title: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.title,
        weight: FontWeight.w700,
        height: 1.3,
        color: text,
      ),
      titleSmall: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.titleSmall,
        weight: FontWeight.w600,
        height: 1.35,
        color: text,
      ),
      body: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.body,
        height: StrataTypeScale.bodyHeight,
        color: text,
      ),
      bodyStrong: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.body,
        weight: FontWeight.w600,
        height: StrataTypeScale.bodyHeight,
        color: text,
      ),
      bodySmall: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.bodySmall,
        height: StrataTypeScale.bodyHeight,
        color: text,
      ),
      caption: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.caption,
        height: StrataTypeScale.bodyHeight,
        color: text2,
      ),
      label: strataTextStyle(
        family: cairo,
        size: StrataTypeScale.bodySmall,
        weight: FontWeight.w600,
        height: 1.4,
        color: text,
      ),
      mono: strataTextStyle(
        family: StrataFonts.plexMono,
        size: 13,
        height: 1.4,
        color: text,
      ),
      monoSmall: strataTextStyle(
        family: StrataFonts.plexMono,
        size: 12,
        weight: FontWeight.w500,
        height: 1.4,
        color: text2,
      ),
      wordmark: strataTextStyle(
        family: StrataFonts.quicksand,
        size: 22,
        weight: FontWeight.w700,
        height: 1,
        color: text,
      ),
    );
  }

  /// Display: 28 / 700.
  final TextStyle display;

  /// Title: 22 / 700.
  final TextStyle title;

  /// Small title: 18 / 600.
  final TextStyle titleSmall;

  /// Body: 16 / 400, line height 1.5.
  final TextStyle body;

  /// Body emphasis: 16 / 600.
  final TextStyle bodyStrong;

  /// Small body: 14 / 400.
  final TextStyle bodySmall;

  /// Caption: 12 / 400 in the secondary colour.
  final TextStyle caption;

  /// Labels on buttons, chips and nav items: 14 / 600.
  final TextStyle label;

  /// Technical text: Plex Mono 13.
  final TextStyle mono;

  /// Small technical text (keyboard hints, IDs): Plex Mono 12 / 500.
  final TextStyle monoSmall;

  /// Wordmark: Quicksand 22 / 700.
  final TextStyle wordmark;

  /// A Material [TextTheme] mapped onto the Strata scale.
  TextTheme toTextTheme() {
    return TextTheme(
      displayLarge: display,
      displayMedium: display,
      displaySmall: display,
      headlineLarge: display,
      headlineMedium: title,
      headlineSmall: title,
      titleLarge: title,
      titleMedium: titleSmall,
      titleSmall: bodyStrong,
      bodyLarge: body,
      bodyMedium: bodySmall,
      bodySmall: caption,
      labelLarge: label,
      labelMedium: label.copyWith(fontSize: StrataTypeScale.caption),
      labelSmall: caption.withWeight(FontWeight.w500),
    );
  }

  @override
  StrataTextStyles copyWith({
    TextStyle? display,
    TextStyle? title,
    TextStyle? titleSmall,
    TextStyle? body,
    TextStyle? bodyStrong,
    TextStyle? bodySmall,
    TextStyle? caption,
    TextStyle? label,
    TextStyle? mono,
    TextStyle? monoSmall,
    TextStyle? wordmark,
  }) {
    return StrataTextStyles(
      display: display ?? this.display,
      title: title ?? this.title,
      titleSmall: titleSmall ?? this.titleSmall,
      body: body ?? this.body,
      bodyStrong: bodyStrong ?? this.bodyStrong,
      bodySmall: bodySmall ?? this.bodySmall,
      caption: caption ?? this.caption,
      label: label ?? this.label,
      mono: mono ?? this.mono,
      monoSmall: monoSmall ?? this.monoSmall,
      wordmark: wordmark ?? this.wordmark,
    );
  }

  @override
  StrataTextStyles lerp(StrataTextStyles? other, double t) {
    if (other == null) return this;
    TextStyle l(TextStyle a, TextStyle b) => TextStyle.lerp(a, b, t)!;
    return StrataTextStyles(
      display: l(display, other.display),
      title: l(title, other.title),
      titleSmall: l(titleSmall, other.titleSmall),
      body: l(body, other.body),
      bodyStrong: l(bodyStrong, other.bodyStrong),
      bodySmall: l(bodySmall, other.bodySmall),
      caption: l(caption, other.caption),
      label: l(label, other.label),
      mono: l(mono, other.mono),
      monoSmall: l(monoSmall, other.monoSmall),
      wordmark: l(wordmark, other.wordmark),
    );
  }
}

/// Weight changes that keep the variable-font `wght` axis in sync.
extension StrataTextStyleWeight on TextStyle {
  /// This style at [weight], applied to both [TextStyle.fontWeight] and the
  /// `wght` font variation.
  TextStyle withWeight(FontWeight weight) => copyWith(
    fontWeight: weight,
    fontVariations: [FontVariation.weight(weight.value.toDouble())],
  );
}
