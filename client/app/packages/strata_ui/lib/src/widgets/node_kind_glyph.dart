import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/graph.dart';
import 'package:strata_ui/src/widgets/labels.dart';

/// The graph glyph of a node kind (note, concept, person, company, document,
/// place) in its shape and colour, used in legends, lists and chips.
class NodeKindGlyph extends StatelessWidget {
  /// Creates the glyph.
  const new({
    super.key,
    required this.kind,
    this.size = 16,
    this.selected = false,
    this.decorative = false,
  });

  /// The node kind.
  final NodeKind kind;

  /// Edge length of the glyph box.
  final double size;

  /// Draws the 3 px tide selection ring.
  final bool selected;

  /// Hides the glyph from semantics when its meaning is already in adjacent
  /// text; otherwise it is announced by its kind name.
  final bool decorative;

  @override
  Widget build(BuildContext context) {
    final graph = context.strataGraphColors;
    final painted = CustomPaint(
      size: Size.square(size),
      painter: NodeKindPainter(
        shape: nodeShapeOf(kind),
        colors: graph.node(kind),
        selectedRing: selected ? graph.selectedRing : null,
      ),
    );
    if (decorative) return ExcludeSemantics(child: painted);
    return Semantics(
      label: context.l10n.nodeKindLabel(kind),
      image: true,
      selected: selected,
      child: painted,
    );
  }
}

/// Paints a [NodeShape].
class NodeKindPainter extends CustomPainter {
  /// Creates the painter.
  const new({required this.shape, required this.colors, this.selectedRing});

  /// The shape.
  final NodeShape shape;

  /// Fill and stroke.
  final NodeKindColors colors;

  /// Colour of the selection ring, or `null` when not selected.
  final Color? selectedRing;

  @override
  void paint(Canvas canvas, Size size) {
    final ring = selectedRing;
    final inset = ring == null ? 1.0 : 4.0;
    final box = (Offset.zero & size).deflate(inset);
    final fill = colors.fill;
    final fillPaint = Paint()
      ..style = PaintingStyle.fill
      ..color = fill ?? Colors.transparent;
    final strokePaint = Paint()
      ..style = PaintingStyle.stroke
      ..color = colors.stroke
      ..strokeWidth = 2
      ..strokeJoin = StrokeJoin.round
      ..strokeCap = StrokeCap.round;
    final c = box.center;
    final r = box.shortestSide / 2;

    switch (shape) {
      case NodeShape.circle:
        canvas.drawCircle(c, r, fillPaint);
      case NodeShape.diamond:
        final d = Path()
          ..moveTo(c.dx, c.dy - r + 1)
          ..lineTo(c.dx + r - 1, c.dy)
          ..lineTo(c.dx, c.dy + r - 1)
          ..lineTo(c.dx - r + 1, c.dy)
          ..close();
        canvas
          ..drawPath(d, fillPaint)
          ..drawPath(d, strokePaint);
      case NodeShape.ringedCircle:
        // Inner disc, 2 px gap, 1.5 px outer ring.
        final outer = Paint()
          ..style = PaintingStyle.stroke
          ..color = colors.stroke
          ..strokeWidth = 1.5;
        canvas
          ..drawCircle(c, r - 0.75, outer)
          ..drawCircle(c, math.max(1, r - 0.75 - 0.75 - 2), fillPaint);
      case NodeShape.roundedSquare:
        final side = r * 2 * 0.86;
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromCenter(center: c, width: side, height: side),
            Radius.circular(side * 0.22),
          ),
          fillPaint,
        );
      case NodeShape.foldedPage:
        final w = r * 2 * 0.78;
        final h = r * 2;
        final left = c.dx - w / 2;
        final top = c.dy - h / 2;
        final fold = w * 0.36;
        final page = Path()
          ..moveTo(left, top)
          ..lineTo(left + w - fold, top)
          ..lineTo(left + w, top + fold)
          ..lineTo(left + w, top + h)
          ..lineTo(left, top + h)
          ..close();
        canvas.drawPath(page, fillPaint);
        final corner = Path()
          ..moveTo(left + w - fold, top)
          ..lineTo(left + w - fold, top + fold)
          ..lineTo(left + w, top + fold);
        canvas.drawPath(
          corner,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1.2
            ..strokeJoin = StrokeJoin.round
            ..color = Colors.white.withValues(alpha: 0.7),
        );
      case NodeShape.pin:
        final pinR = r * 0.62;
        final head = Offset(c.dx, box.top + pinR + 1);
        final pin = Path()
          ..moveTo(c.dx, box.bottom - 0.5)
          ..quadraticBezierTo(
            c.dx - pinR * 1.05,
            head.dy + pinR * 0.9,
            c.dx - pinR,
            head.dy,
          )
          ..arcToPoint(
            Offset(c.dx + pinR, head.dy),
            radius: Radius.circular(pinR),
          )
          ..quadraticBezierTo(
            c.dx + pinR * 1.05,
            head.dy + pinR * 0.9,
            c.dx,
            box.bottom - 0.5,
          )
          ..close();
        canvas
          ..drawPath(pin, strokePaint..strokeWidth = 1.75)
          ..drawCircle(head, pinR * 0.35, Paint()..color = colors.stroke);
    }

    if (ring != null) {
      canvas.drawCircle(
        size.center(Offset.zero),
        size.shortestSide / 2 - 1.5,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 3
          ..color = ring,
      );
    }
  }

  @override
  bool shouldRepaint(NodeKindPainter oldDelegate) =>
      oldDelegate.shape != shape ||
      oldDelegate.colors.fill != colors.fill ||
      oldDelegate.colors.stroke != colors.stroke ||
      oldDelegate.selectedRing != selectedRing;
}
