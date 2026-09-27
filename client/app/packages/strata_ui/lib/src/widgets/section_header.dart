import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';

/// A section heading ("Pinned", "Backlinks · supports", "Today") with an
/// optional count and trailing action. Announced as a header.
class StrataSectionHeader extends StatelessWidget {
  /// Creates a section header.
  const new({
    super.key,
    required this.title,
    this.count,
    this.trailing,
    this.padding = const EdgeInsetsDirectional.fromSTEB(
      StrataSpacing.s4,
      StrataSpacing.s3,
      StrataSpacing.s2,
      StrataSpacing.s1,
    ),
  });

  /// Section title.
  final String title;

  /// Optional item count.
  final int? count;

  /// Optional trailing action (e.g. a text button).
  final Widget? trailing;

  /// Outer padding.
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final style = context.strataText.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2, letterSpacing: 0.4);
    final n = count;
    final end = trailing;
    return Padding(
      padding: padding,
      child: Row(
        children: [
          Expanded(
            child: Semantics(
              header: true,
              label: n == null ? title : '$title, $n',
              excludeSemantics: true,
              child: Text.rich(
                TextSpan(
                  children: [
                    TextSpan(text: title),
                    if (n != null)
                      TextSpan(
                        text: '  $n',
                        style: style.withWeight(FontWeight.w500),
                      ),
                  ],
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: style,
              ),
            ),
          ),
          ?end,
        ],
      ),
    );
  }
}
