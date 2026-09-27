import 'package:flutter/material.dart';

/// The raw "coastal" brand palette (design/README.md, design/SCREEN_SPEC.md).
///
/// Widgets never read these directly; they read the role-based
/// [StrataColors] from the theme so light and dark stay consistent.
abstract final class StrataPalette {
  /// Tide: the lead colour (the mark, icons, focus rings, selected
  /// indicators and large graphics). Not a fill behind small text: white on
  /// tide is 4.8:1 at rest and drops below 4.5:1 under the hover and pressed
  /// overlays, so filled buttons use [tideText] (light) or [surf] (dark).
  static const Color tide = Color(0xFF2477B3);

  /// Text-safe tide: links and small accent text on light grounds, and the
  /// light-theme fill of filled/primary buttons (white on it is 7.1:1).
  static const Color tideText = Color(0xFF1D5C8C);

  /// Tide text hover state on light grounds.
  static const Color tideTextHover = Color(0xFF164A72);

  /// Abyss: ink, app icon background, dark UI ground.
  static const Color abyss = Color(0xFF0F1B26);

  /// Mist: light backgrounds; the mark on dark.
  static const Color mist = Color(0xFFF1F5F7);

  /// Harbour: dividers and quiet UI (icons, text only at 16 px and above).
  static const Color harbour = Color(0xFF7C8C96);

  /// Harbour for text (the minimum grey for small text on light grounds).
  static const Color harbourText = Color(0xFF52616B);

  /// Surf: small text and links on dark grounds, and the dark-theme fill of
  /// filled/primary buttons (abyss on it is 7.6:1).
  static const Color surf = Color(0xFF6CB4DD);

  /// Sand: only the thin seam in the strata bands.
  static const Color sand = Color(0xFFD8B47E);

  /// Pure white (surfaces in light theme, text on tide).
  static const Color white = Color(0xFFFFFFFF);
}

/// Role-based colour tokens for one brightness, as a [ThemeExtension].
///
/// Light values come from the design spec; dark semantic values not given by
/// the spec (dark tints and danger) are derived to keep WCAG AA contrast and
/// are verified by tests.
///
/// Fills that carry text have their own foreground token, and every such
/// pair is at least 4.5:1 (also under the Material hover, focus and pressed
/// overlays; `test/tokens/contrast_test.dart`):
///
/// - [accentFill] / [onAccentFill]: filled/primary buttons, the FAB and
///   `ColorScheme.primary`/`onPrimary`. Light `#1D5C8C` / white (7.1:1),
///   dark `#6CB4DD` / `#0F1B26` (7.6:1).
/// - [danger] / [onDanger]: destructive buttons and
///   `ColorScheme.error`/`onError`. Light `#B3412E` / white (5.7:1), dark
///   `#E07A66` / `#0F1B26` (5.9:1).
/// - [accent] / [onAccent]: tide `#2477B3` / white (4.8:1 at rest) in both
///   themes, for icons and text of 18 px and above only.
///
/// Destructive buttons use the standard error style:
/// `FilledButton.styleFrom(backgroundColor: scheme.error,
/// foregroundColor: scheme.onError)`.
@immutable
class StrataColors extends ThemeExtension<StrataColors> {
  /// Creates a colour token set.
  const new({
    required this.background,
    required this.surface,
    required this.surface2,
    required this.border,
    required this.text,
    required this.text2,
    required this.text3,
    required this.accent,
    required this.onAccent,
    required this.accentFill,
    required this.onAccentFill,
    required this.accentText,
    required this.accentTint,
    required this.sand,
    required this.success,
    required this.successTint,
    required this.successText,
    required this.warning,
    required this.warningTint,
    required this.warningText,
    required this.danger,
    required this.onDanger,
    required this.dangerTint,
    required this.dangerText,
    required this.info,
    required this.infoTint,
    required this.infoText,
    required this.scrim,
  });

  /// Light theme tokens (default).
  static const StrataColors light = StrataColors(
    background: StrataPalette.mist,
    surface: StrataPalette.white,
    surface2: Color(0xFFE8EFF2),
    border: Color(0xFFCBD8DE),
    text: StrataPalette.abyss,
    text2: StrataPalette.harbourText,
    text3: StrataPalette.harbour,
    accent: StrataPalette.tide,
    onAccent: StrataPalette.white,
    accentFill: StrataPalette.tideText,
    onAccentFill: StrataPalette.white,
    accentText: StrataPalette.tideText,
    accentTint: Color(0xFFDCEAF4),
    sand: StrataPalette.sand,
    success: Color(0xFF2F7A55),
    successTint: Color(0xFFEAF4EE),
    successText: Color(0xFF2F7A55),
    warning: Color(0xFF9A6A12),
    warningTint: Color(0xFFF5ECDA),
    warningText: Color(0xFF6E4B0C),
    danger: Color(0xFFB3412E),
    onDanger: StrataPalette.white,
    dangerTint: Color(0xFFF6E3DF),
    dangerText: Color(0xFF9A3624),
    info: StrataPalette.tide,
    infoTint: Color(0xFFDCEAF4),
    infoText: StrataPalette.tideText,
    scrim: Color(0x520F1B26),
  );

  /// Dark theme tokens.
  static const StrataColors dark = StrataColors(
    background: StrataPalette.abyss,
    surface: Color(0xFF15283A),
    surface2: Color(0xFF1B3044),
    border: Color(0xFF23384A),
    text: StrataPalette.mist,
    text2: Color(0xFF93A3AD),
    text3: StrataPalette.harbour,
    accent: StrataPalette.tide,
    onAccent: StrataPalette.white,
    accentFill: StrataPalette.surf,
    onAccentFill: StrataPalette.abyss,
    accentText: StrataPalette.surf,
    accentTint: Color(0xFF1B3A55),
    sand: StrataPalette.sand,
    success: Color(0xFF6CC495),
    successTint: Color(0xFF173A2C),
    successText: Color(0xFF8FD6AE),
    warning: Color(0xFFC99A3E),
    warningTint: Color(0xFF3A2E14),
    warningText: Color(0xFFE3BC6E),
    danger: Color(0xFFE07A66),
    onDanger: StrataPalette.abyss,
    dangerTint: Color(0xFF3F1F1A),
    dangerText: Color(0xFFF0A392),
    info: StrataPalette.surf,
    infoTint: Color(0xFF1B3A55),
    infoText: StrataPalette.surf,
    scrim: Color(0x99060D13),
  );

  /// App background ("mist" in light).
  final Color background;

  /// Cards and panes.
  final Color surface;

  /// Sidebar, rail, hover rows, inputs.
  final Color surface2;

  /// 1 px hairlines.
  final Color border;

  /// Primary text.
  final Color text;

  /// Secondary text; the minimum for small text.
  final Color text2;

  /// Tertiary: only for text of 16 px and above, or icons.
  final Color text3;

  /// Accent (tide, both themes): the mark, icons, focus rings, selected
  /// indicators and large graphics. Filled buttons use [accentFill].
  final Color accent;

  /// Icons and large text (18 px and above) on [accent].
  final Color onAccent;

  /// Fill of filled/primary buttons and the FAB (`ColorScheme.primary`):
  /// text-safe tide `#1D5C8C` in light, surf `#6CB4DD` in dark.
  final Color accentFill;

  /// Text and icons on [accentFill] (`ColorScheme.onPrimary`): white in
  /// light, abyss in dark; at least 4.5:1 in every button state.
  final Color onAccentFill;

  /// Accent for text and links.
  final Color accentText;

  /// Accent tint: selected rows and navigation pills.
  final Color accentTint;

  /// Sand: the strata-band seam only.
  final Color sand;

  /// Success signal colour (icons, dots).
  final Color success;

  /// Success tint background.
  final Color successTint;

  /// Text on [successTint].
  final Color successText;

  /// Warning signal colour.
  final Color warning;

  /// Warning tint background.
  final Color warningTint;

  /// Text on [warningTint].
  final Color warningText;

  /// Danger signal colour, error text on surfaces and the fill of
  /// destructive buttons (`ColorScheme.error`).
  final Color danger;

  /// Text and icons on [danger] (`ColorScheme.onError`): white in light,
  /// abyss in dark (white on the dark danger would be 2.9:1).
  final Color onDanger;

  /// Danger tint background.
  final Color dangerTint;

  /// Text on [dangerTint].
  final Color dangerText;

  /// Info signal colour.
  final Color info;

  /// Info tint background (the accent tint).
  final Color infoTint;

  /// Text on [infoTint].
  final Color infoText;

  /// Modal barrier colour behind drawers and sheets.
  final Color scrim;

  @override
  StrataColors copyWith({
    Color? background,
    Color? surface,
    Color? surface2,
    Color? border,
    Color? text,
    Color? text2,
    Color? text3,
    Color? accent,
    Color? onAccent,
    Color? accentFill,
    Color? onAccentFill,
    Color? accentText,
    Color? accentTint,
    Color? sand,
    Color? success,
    Color? successTint,
    Color? successText,
    Color? warning,
    Color? warningTint,
    Color? warningText,
    Color? danger,
    Color? onDanger,
    Color? dangerTint,
    Color? dangerText,
    Color? info,
    Color? infoTint,
    Color? infoText,
    Color? scrim,
  }) {
    return StrataColors(
      background: background ?? this.background,
      surface: surface ?? this.surface,
      surface2: surface2 ?? this.surface2,
      border: border ?? this.border,
      text: text ?? this.text,
      text2: text2 ?? this.text2,
      text3: text3 ?? this.text3,
      accent: accent ?? this.accent,
      onAccent: onAccent ?? this.onAccent,
      accentFill: accentFill ?? this.accentFill,
      onAccentFill: onAccentFill ?? this.onAccentFill,
      accentText: accentText ?? this.accentText,
      accentTint: accentTint ?? this.accentTint,
      sand: sand ?? this.sand,
      success: success ?? this.success,
      successTint: successTint ?? this.successTint,
      successText: successText ?? this.successText,
      warning: warning ?? this.warning,
      warningTint: warningTint ?? this.warningTint,
      warningText: warningText ?? this.warningText,
      danger: danger ?? this.danger,
      onDanger: onDanger ?? this.onDanger,
      dangerTint: dangerTint ?? this.dangerTint,
      dangerText: dangerText ?? this.dangerText,
      info: info ?? this.info,
      infoTint: infoTint ?? this.infoTint,
      infoText: infoText ?? this.infoText,
      scrim: scrim ?? this.scrim,
    );
  }

  @override
  StrataColors lerp(StrataColors? other, double t) {
    if (other == null) return this;
    Color l(Color a, Color b) => Color.lerp(a, b, t)!;
    return StrataColors(
      background: l(background, other.background),
      surface: l(surface, other.surface),
      surface2: l(surface2, other.surface2),
      border: l(border, other.border),
      text: l(text, other.text),
      text2: l(text2, other.text2),
      text3: l(text3, other.text3),
      accent: l(accent, other.accent),
      onAccent: l(onAccent, other.onAccent),
      accentFill: l(accentFill, other.accentFill),
      onAccentFill: l(onAccentFill, other.onAccentFill),
      accentText: l(accentText, other.accentText),
      accentTint: l(accentTint, other.accentTint),
      sand: l(sand, other.sand),
      success: l(success, other.success),
      successTint: l(successTint, other.successTint),
      successText: l(successText, other.successText),
      warning: l(warning, other.warning),
      warningTint: l(warningTint, other.warningTint),
      warningText: l(warningText, other.warningText),
      danger: l(danger, other.danger),
      onDanger: l(onDanger, other.onDanger),
      dangerTint: l(dangerTint, other.dangerTint),
      dangerText: l(dangerText, other.dangerText),
      info: l(info, other.info),
      infoTint: l(infoTint, other.infoTint),
      infoText: l(infoText, other.infoText),
      scrim: l(scrim, other.scrim),
    );
  }
}
