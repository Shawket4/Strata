import 'dart:math' as math;
import 'dart:ui';

import 'package:strata_ui/strata_ui.dart';

/// Draws one edge from [a] to [b] (screen space) in [style] and [color]:
/// solid, dashed or dotted, double, with an arrow head and a mid-point ×
/// when [details] is on (GraphLanguage: arrowheads and × marks appear at
/// near zoom). [trimEnd] shortens the end so arrows stop short of the node.
void drawStyledEdge(
  Canvas canvas,
  Offset a,
  Offset b,
  RelationLineStyle style,
  Color color, {
  bool details = true,
  double trimStart = 0,
  double trimEnd = 0,
  double widthScale = 1,
  bool squareEnd = false,
}) {
  final delta = b - a;
  final length = delta.distance;
  if (length <= trimStart + trimEnd + 1) return;
  final dir = delta / length;
  final start = a + dir * trimStart;
  final end = b - dir * trimEnd;
  final paint = Paint()
    ..color = color.withValues(alpha: color.a * style.opacity)
    ..style = PaintingStyle.stroke
    ..strokeWidth = style.strokeWidth * widthScale
    ..strokeCap = StrokeCap.butt;
  final normal = Offset(-dir.dy, dir.dx);
  final pattern = style.dashPattern;
  void line(Offset p, Offset q) {
    if (pattern == null || pattern.isEmpty) {
      canvas.drawLine(p, q, paint);
    } else {
      _dashed(canvas, p, q, pattern, paint);
    }
  }

  if (style.doubleLine) {
    line(start + normal * 1.75, end + normal * 1.75);
    line(start - normal * 1.75, end - normal * 1.75);
  } else {
    line(start, end);
  }
  if (!details) return;
  final solid = Paint()
    ..color = paint.color
    ..style = PaintingStyle.stroke
    ..strokeWidth = math.max(1.5, paint.strokeWidth)
    ..strokeCap = StrokeCap.round
    ..strokeJoin = StrokeJoin.round;
  if (style.arrow) {
    const head = 6.0;
    final back = end - dir * head;
    canvas.drawPath(
      Path()
        ..moveTo(
          back.dx + normal.dx * head * 0.6,
          back.dy + normal.dy * head * 0.6,
        )
        ..lineTo(end.dx, end.dy)
        ..lineTo(
          back.dx - normal.dx * head * 0.6,
          back.dy - normal.dy * head * 0.6,
        ),
      solid,
    );
  }
  if (squareEnd) {
    canvas.drawRect(
      Rect.fromCenter(center: end, width: 5, height: 5),
      Paint()..color = paint.color,
    );
  }
  if (style.midCross) {
    final mid = (start + end) / 2;
    const r = 3.5;
    final u = (dir + normal) * (r / math.sqrt2);
    final v = (dir - normal) * (r / math.sqrt2);
    canvas
      ..drawLine(mid - u, mid + u, solid)
      ..drawLine(mid - v, mid + v, solid);
  }
}

void _dashed(
  Canvas canvas,
  Offset a,
  Offset b,
  List<double> pattern,
  Paint paint,
) {
  final delta = b - a;
  final length = delta.distance;
  if (length == 0) return;
  final dir = delta / length;
  var distance = 0.0;
  var i = 0;
  while (distance < length) {
    final segment = pattern[i % pattern.length];
    if (i.isEven) {
      final from = a + dir * distance;
      final to = a + dir * math.min(distance + segment, length);
      canvas.drawLine(from, to, paint);
    }
    distance += segment;
    i++;
  }
}
