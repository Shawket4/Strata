import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// An avatar drawn as initials (SCREEN_SPEC: "draw avatars as initials in
/// circles"): a circle for people and accounts, a rounded square for
/// companies ([square]).
///
/// The initials come from the core (`initials` on accounts, users and
/// entities); an empty string shows [fallbackIcon] instead. Decorative: the
/// name next to it carries the semantics.
class StrataAvatar extends StatelessWidget {
  /// Creates the avatar.
  const new({
    required this.initials,
    super.key,
    this.size = 40,
    this.square = false,
    this.fallbackIcon = Icons.person_outline,
  });

  /// One or two letters from the core.
  final String initials;

  /// Diameter.
  final double size;

  /// A rounded square (companies) instead of a circle.
  final bool square;

  /// Shown when [initials] is empty.
  final IconData fallbackIcon;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return ExcludeSemantics(
      child: Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: colors.accentTint,
          shape: square ? BoxShape.rectangle : BoxShape.circle,
          borderRadius: square ? BorderRadius.circular(size * 0.25) : null,
        ),
        child: initials.isEmpty
            ? Icon(fallbackIcon, size: size * 0.55, color: colors.accentText)
            : FittedBox(
                child: Padding(
                  padding: EdgeInsets.all(size * 0.12),
                  child: Text(
                    initials,
                    maxLines: 1,
                    textScaler: TextScaler.noScaling,
                    style: text.bodyStrong.copyWith(
                      color: colors.accentText,
                      fontSize: size * 0.38,
                      height: 1,
                    ),
                  ),
                ),
              ),
      ),
    );
  }
}
