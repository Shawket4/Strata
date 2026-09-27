import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/colors.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';

/// Semantic tone of a status pill. Every tone also has its own icon so
/// meaning never depends on colour alone.
enum StatusTone {
  /// Neutral / informational without emphasis.
  neutral,

  /// Informational (tide tint).
  info,

  /// Success (green tint).
  success,

  /// Warning (amber tint).
  warning,

  /// Danger (red tint).
  danger;

  /// Background and foreground colours of this tone.
  ({Color background, Color foreground}) colorsIn(StrataColors colors) =>
      switch (this) {
        StatusTone.neutral => (
          background: colors.surface2,
          foreground: colors.text2,
        ),
        StatusTone.info => (
          background: colors.infoTint,
          foreground: colors.infoText,
        ),
        StatusTone.success => (
          background: colors.successTint,
          foreground: colors.successText,
        ),
        StatusTone.warning => (
          background: colors.warningTint,
          foreground: colors.warningText,
        ),
        StatusTone.danger => (
          background: colors.dangerTint,
          foreground: colors.dangerText,
        ),
      };

  /// The default icon of this tone.
  IconData get icon => switch (this) {
    StatusTone.neutral => Icons.circle_outlined,
    StatusTone.info => Icons.info_outline,
    StatusTone.success => Icons.check_circle_outline,
    StatusTone.warning => Icons.warning_amber_outlined,
    StatusTone.danger => Icons.error_outline,
  };
}

/// A small tinted pill with an icon and a label (e.g. "Pending approval",
/// "AI paused", "Disabled").
class StatusPill extends StatelessWidget {
  /// Creates a status pill.
  const new({
    required this.label,
    super.key,
    this.tone = StatusTone.neutral,
    this.icon,
    this.trailing,
  });

  /// The status text.
  final String label;

  /// Semantic tone.
  final StatusTone tone;

  /// Overrides the tone's default icon.
  final IconData? icon;

  /// Optional trailing widget (e.g. a progress indicator).
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final palette = tone.colorsIn(context.strataColors);
    final style = context.strataText.caption
        .withWeight(FontWeight.w600)
        .copyWith(color: palette.foreground);
    final extra = trailing;
    return Semantics(
      container: true,
      label: label,
      excludeSemantics: true,
      child: Container(
        constraints: const BoxConstraints(minHeight: 24),
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s2 + 2,
          vertical: 2,
        ),
        decoration: BoxDecoration(
          color: palette.background,
          borderRadius: StrataRadii.pillRadius,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon ?? tone.icon, size: 14, color: palette.foreground),
            const SizedBox(width: StrataSpacing.s1 + 2),
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: style,
              ),
            ),
            if (extra != null) ...[
              const SizedBox(width: StrataSpacing.s1 + 2),
              extra,
            ],
          ],
        ),
      ),
    );
  }
}
