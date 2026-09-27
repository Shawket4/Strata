import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';
import 'package:strata_ui/src/widgets/tap_target.dart';

/// A citation (`[[Note#^block]]`) rendered as a chip in Ask answers and AI
/// bullets; activating it opens the cited block.
class CitationChip extends StatelessWidget {
  /// Creates a citation chip.
  const new({
    super.key,
    required this.label,
    this.blockRef,
    this.index,
    this.onPressed,
    this.textDirection,
  });

  /// Title of the cited note.
  final String label;

  /// Block reference, e.g. `^a1b2`.
  final String? blockRef;

  /// Citation number shown before the title.
  final int? index;

  /// Opens the cited block.
  final VoidCallback? onPressed;

  /// Direction of [label]; defaults to the ambient direction.
  final TextDirection? textDirection;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final n = index;
    final ref = blockRef;
    final chip = Container(
      constraints: const BoxConstraints(minHeight: 28),
      padding: const EdgeInsetsDirectional.fromSTEB(
        StrataSpacing.s1,
        2,
        StrataSpacing.s3,
        2,
      ),
      decoration: BoxDecoration(
        color: colors.infoTint,
        borderRadius: StrataRadii.pillRadius,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (n != null)
            Container(
              constraints: const BoxConstraints(minWidth: 20, minHeight: 20),
              alignment: Alignment.center,
              padding: const EdgeInsets.symmetric(horizontal: 4),
              decoration: BoxDecoration(
                color: colors.surface,
                borderRadius: StrataRadii.pillRadius,
              ),
              child: Text(
                '$n',
                style: text.monoSmall.copyWith(color: colors.infoText),
              ),
            )
          else
            Icon(Icons.format_quote_rounded, size: 16, color: colors.infoText),
          const SizedBox(width: StrataSpacing.s1 + 2),
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              textDirection: textDirection,
              style: text.bodySmall
                  .withWeight(FontWeight.w600)
                  .copyWith(color: colors.infoText),
            ),
          ),
          if (ref != null) ...[
            const SizedBox(width: StrataSpacing.s1),
            Text(
              '#$ref',
              maxLines: 1,
              textDirection: TextDirection.ltr,
              style: text.monoSmall.copyWith(color: colors.infoText),
            ),
          ],
        ],
      ),
    );
    return StrataTapTarget(
      semanticLabel: context.l10n.citationSemantics(label: label),
      onTap: onPressed,
      child: chip,
    );
  }
}
