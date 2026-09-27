import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/graph.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';
import 'package:strata_ui/src/widgets/labels.dart';
import 'package:strata_ui/src/widgets/tap_target.dart';

/// A typed relation chip: line-sample glyph + label, plus an "AI · 0.82" tag
/// for AI-made relations (user-made relations show nothing extra).
class RelationChip extends StatelessWidget {
  /// Creates a relation chip.
  const new({
    required this.type,
    required this.label,
    super.key,
    this.aiConfidence,
    this.mentionOf = NodeKind.person,
    this.onPressed,
    this.onLongPress,
    this.textDirection,
  });

  /// The relation type.
  final RelationType type;

  /// The related note or entity title.
  final String label;

  /// Confidence of an AI-made relation; `null` for user-made relations.
  final double? aiConfidence;

  /// For [RelationType.mention], the mentioned entity's kind (colour).
  final NodeKind mentionOf;

  /// Opens the related note.
  final VoidCallback? onPressed;

  /// Opens the reject / retype sheet (phones).
  final VoidCallback? onLongPress;

  /// Direction of [label] (content direction comes from the core, per
  /// paragraph); defaults to the ambient direction.
  final TextDirection? textDirection;

  /// The visible AI tag text for [confidence], e.g. `AI · 0.82`.
  static String aiTagText(String aiLabel, double confidence) =>
      '$aiLabel · ${confidence.toStringAsFixed(2)}';

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final confidence = aiConfidence;
    final typeLabel = l10n.relationTypeLabel(type);
    final semantic = StringBuffer('$typeLabel: $label');
    if (confidence != null) {
      final value = confidence.toStringAsFixed(2);
      semantic.write(', ${l10n.aiConfidenceSemantics(confidence: value)}');
    }
    final chip = Container(
      constraints: const BoxConstraints(minHeight: 28),
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s3,
        vertical: StrataSpacing.s1,
      ),
      decoration: BoxDecoration(
        color: colors.surface,
        border: Border.all(color: colors.border),
        borderRadius: StrataRadii.pillRadius,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          RelationLineSample(type: type, mentionOf: mentionOf),
          const SizedBox(width: StrataSpacing.s2),
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              textDirection: textDirection,
              style: text.bodySmall.withWeight(FontWeight.w500),
            ),
          ),
          if (confidence != null) ...[
            const SizedBox(width: StrataSpacing.s2),
            Flexible(
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: StrataSpacing.s2 - 2,
                ),
                decoration: BoxDecoration(
                  color: colors.infoTint,
                  borderRadius: StrataRadii.pillRadius,
                ),
                child: Text(
                  aiTagText(l10n.aiTag, confidence),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: text.caption
                      .withWeight(FontWeight.w600)
                      .copyWith(color: colors.infoText),
                ),
              ),
            ),
          ],
        ],
      ),
    );
    return StrataTapTarget(
      semanticLabel: semantic.toString(),
      onTap: onPressed,
      onLongPress: onLongPress,
      child: chip,
    );
  }
}

/// A small line sample drawn in a relation type's colour and pattern (the
/// chip glyph and the legend swatch). Mirrors in right-to-left layouts so
/// arrows point in reading direction.
class RelationLineSample extends StatelessWidget {
  /// Creates a line sample.
  const new({
    required this.type,
    super.key,
    this.mentionOf = NodeKind.person,
    this.width = 20,
    this.height = 12,
  });

  /// The relation type.
  final RelationType type;

  /// For mentions, the entity kind whose colour is used.
  final NodeKind mentionOf;

  /// Sample width.
  final double width;

  /// Sample height.
  final double height;

  @override
  Widget build(BuildContext context) {
    final graph = context.strataGraphColors;
    return ExcludeSemantics(
      child: CustomPaint(
        size: Size(width, height),
        painter: RelationLinePainter(
          style: RelationLineStyle.of(type),
          color: graph.relation(type, mentionOf: mentionOf),
          textDirection: Directionality.of(context),
        ),
      ),
    );
  }
}

/// Paints a horizontal sample of a [RelationLineStyle].
class RelationLinePainter extends CustomPainter {
  /// Creates the painter.
  const new({
    required this.style,
    required this.color,
    this.textDirection = TextDirection.ltr,
  });

  /// The line style.
  final RelationLineStyle style;

  /// The base colour (the style's opacity is applied on top).
  final Color color;

  /// Arrows point towards the end edge.
  final TextDirection textDirection;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color.withValues(alpha: color.a * style.opacity)
      ..style = PaintingStyle.stroke
      ..strokeWidth = style.strokeWidth
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    if (textDirection == TextDirection.rtl) {
      canvas
        ..save()
        ..translate(size.width, 0)
        ..scale(-1, 1);
    }
    final y = size.height / 2;
    final start = style.strokeWidth;
    final end = size.width - style.strokeWidth;
    final lineEnd = style.arrow ? end - 1 : end;
    if (style.doubleLine) {
      const gap = 2.0;
      _drawLine(
        canvas,
        paint,
        Offset(start, y - gap),
        Offset(lineEnd, y - gap),
      );
      _drawLine(
        canvas,
        paint,
        Offset(start, y + gap),
        Offset(lineEnd, y + gap),
      );
    } else {
      _drawLine(canvas, paint, Offset(start, y), Offset(lineEnd, y));
    }
    if (style.arrow) {
      final head = size.height * 0.3;
      canvas.drawPath(
        Path()
          ..moveTo(end - head, y - head)
          ..lineTo(end, y)
          ..lineTo(end - head, y + head),
        paint..strokeWidth = style.strokeWidth.clamp(1.5, 2),
      );
    }
    if (style.midCross) {
      final mid = (start + lineEnd) / 2;
      const r = 2.5;
      final cross = Paint()
        ..color = paint.color
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..strokeCap = StrokeCap.round;
      canvas
        ..drawLine(Offset(mid - r, y - r), Offset(mid + r, y + r), cross)
        ..drawLine(Offset(mid - r, y + r), Offset(mid + r, y - r), cross);
    }
    if (textDirection == TextDirection.rtl) canvas.restore();
  }

  void _drawLine(Canvas canvas, Paint paint, Offset a, Offset b) {
    final pattern = style.dashPattern;
    final path = Path()
      ..moveTo(a.dx, a.dy)
      ..lineTo(b.dx, b.dy);
    if (pattern == null || pattern.isEmpty) {
      canvas.drawPath(path, paint);
      return;
    }
    final dashed = Path();
    for (final metric in path.computeMetrics()) {
      var distance = 0.0;
      var i = 0;
      while (distance < metric.length) {
        final length = pattern[i % pattern.length];
        if (i.isEven) {
          dashed.addPath(
            metric.extractPath(distance, distance + length),
            Offset.zero,
          );
        }
        distance += length;
        i++;
      }
    }
    // Dots (1–2 px dashes) read better with butt caps.
    final dashPaint = Paint()
      ..color = paint.color
      ..style = PaintingStyle.stroke
      ..strokeWidth = paint.strokeWidth
      ..strokeCap = StrokeCap.butt;
    canvas.drawPath(dashed, dashPaint);
  }

  @override
  bool shouldRepaint(RelationLinePainter oldDelegate) =>
      oldDelegate.style != style ||
      oldDelegate.color != color ||
      oldDelegate.textDirection != textDirection;
}
