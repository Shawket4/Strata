import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// The two drawings of the Strata "S" symbol on the 64-unit grid.
enum StrataSymbolForm {
  /// Standard form: `M50 13 H24 A9.5 9.5 0 0 0 24 32 H40 A9.5 9.5 0 0 1 40 51
  /// H14`, stroke 11.
  standard,

  /// Favicon form for 16–32 px: `M49 14 H25 A9 9 0 0 0 25 32 H39 A9 9 0 0 1
  /// 39 50 H15`, stroke 14.
  favicon,
}

/// The Strata symbol, painted as clean vector strokes with round caps and
/// joins (L11). Decorative by default; pass [semanticLabel] when it stands
/// alone and carries meaning.
class StrataSymbol extends StatelessWidget {
  /// Creates the symbol at [size] × [size] logical pixels.
  const new({
    super.key,
    this.size = 24,
    this.color,
    this.form,
    this.semanticLabel,
  });

  /// Edge length of the square symbol.
  final double size;

  /// Stroke colour; defaults to the theme accent (tide).
  final Color? color;

  /// Which drawing to use; defaults to [StrataSymbolForm.favicon] at 32 px and
  /// below, otherwise [StrataSymbolForm.standard].
  final StrataSymbolForm? form;

  /// Accessibility label; `null` hides the symbol from semantics.
  final String? semanticLabel;

  /// The drawing used at [size] when no [form] is given.
  static StrataSymbolForm formFor(double size) =>
      size <= 32 ? StrataSymbolForm.favicon : StrataSymbolForm.standard;

  @override
  Widget build(BuildContext context) {
    final painted = CustomPaint(
      size: Size.square(size),
      painter: StrataSymbolPainter(
        color: color ?? context.strataColors.accent,
        form: form ?? formFor(size),
      ),
    );
    final label = semanticLabel;
    if (label == null) return ExcludeSemantics(child: painted);
    return Semantics(label: label, image: true, child: painted);
  }
}

/// Paints the Strata symbol scaled from the 64-unit grid to the canvas size.
class StrataSymbolPainter extends CustomPainter {
  /// Creates a painter for [form] in [color].
  const new({required this.color, this.form = StrataSymbolForm.standard});

  /// Stroke colour.
  final Color color;

  /// Which drawing to paint.
  final StrataSymbolForm form;

  /// The design grid size.
  static const double grid = 64;

  /// Stroke width on the 64-unit grid.
  static double strokeWidthOf(StrataSymbolForm form) => switch (form) {
    StrataSymbolForm.standard => 11,
    StrataSymbolForm.favicon => 14,
  };

  /// The symbol path on the 64-unit grid.
  static Path pathOf(StrataSymbolForm form) {
    return switch (form) {
      StrataSymbolForm.standard =>
        Path()
          ..moveTo(50, 13)
          ..lineTo(24, 13)
          ..arcToPoint(
            const Offset(24, 32),
            radius: const Radius.circular(9.5),
            clockwise: false,
          )
          ..lineTo(40, 32)
          ..arcToPoint(const Offset(40, 51), radius: const Radius.circular(9.5))
          ..lineTo(14, 51),
      StrataSymbolForm.favicon =>
        Path()
          ..moveTo(49, 14)
          ..lineTo(25, 14)
          ..arcToPoint(
            const Offset(25, 32),
            radius: const Radius.circular(9),
            clockwise: false,
          )
          ..lineTo(39, 32)
          ..arcToPoint(const Offset(39, 50), radius: const Radius.circular(9))
          ..lineTo(15, 50),
    };
  }

  @override
  void paint(Canvas canvas, Size size) {
    final scale = size.shortestSide / grid;
    canvas
      ..save()
      ..translate(
        (size.width - grid * scale) / 2,
        (size.height - grid * scale) / 2,
      )
      ..scale(scale);
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = strokeWidthOf(form)
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..isAntiAlias = true;
    canvas
      ..drawPath(pathOf(form), paint)
      ..restore();
  }

  @override
  bool shouldRepaint(StrataSymbolPainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.form != form;
}
