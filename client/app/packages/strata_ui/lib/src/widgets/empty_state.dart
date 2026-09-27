import 'package:flutter/material.dart';
import 'package:strata_ui/src/layout/destination.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';

/// An empty or placeholder state: icon, title, optional message and action.
/// Scrolls instead of overflowing at large text scales.
class StrataEmptyState extends StatelessWidget {
  /// Creates an empty state.
  const new({
    required this.title,
    super.key,
    this.message,
    this.icon = Icons.inbox_outlined,
    this.action,
  });

  /// Headline.
  final String title;

  /// Supporting text.
  final String? message;

  /// Illustration icon.
  final IconData icon;

  /// Optional call to action.
  final StrataAction? action;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final body = message;
    final cta = action;
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(StrataSpacing.s6),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ExcludeSemantics(
                child: Container(
                  width: 56,
                  height: 56,
                  decoration: BoxDecoration(
                    color: colors.accentTint,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(icon, size: 26, color: colors.accentText),
                ),
              ),
              const SizedBox(height: StrataSpacing.s4),
              Semantics(
                header: true,
                child: Text(
                  title,
                  textAlign: TextAlign.center,
                  style: text.titleSmall,
                ),
              ),
              if (body != null) ...[
                const SizedBox(height: StrataSpacing.s2),
                Text(
                  body,
                  textAlign: TextAlign.center,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              ],
              if (cta != null) ...[
                const SizedBox(height: StrataSpacing.s5),
                FilledButton.icon(
                  onPressed: cta.onPressed,
                  icon: Icon(cta.icon, size: 18),
                  label: Text(cta.label),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
