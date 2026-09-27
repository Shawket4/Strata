import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// The signature "strata bands": horizontal bands of uneven depth, one tone
/// apart, with a single thin sand seam. Purely decorative (hidden from
/// semantics); an optional [child] is laid on top.
class StrataBands extends StatelessWidget {
  /// Creates the band decoration.
  const new({
    super.key,
    this.child,
    this.depths = defaultDepths,
    this.seamAfter = 2,
    this.baseColor,
    this.toneColor,
  });

  /// Relative depths of the bands from top to bottom (normalised to fill).
  static const List<double> defaultDepths = [0.2, 0.13, 0.24, 0.1, 0.33];

  /// Content drawn over the bands.
  final Widget? child;

  /// Relative band depths, top to bottom.
  final List<double> depths;

  /// Index of the band after which the sand seam is drawn.
  final int seamAfter;

  /// Colour of the top band; defaults to the theme background.
  final Color? baseColor;

  /// Colour each lower band steps towards; defaults to the accent (tide).
  final Color? toneColor;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return CustomPaint(
      painter: StrataBandsPainter(
        depths: depths,
        seamAfter: seamAfter,
        base: baseColor ?? colors.background,
        tone: toneColor ?? colors.accent,
        seam: colors.sand,
      ),
      child: child ?? const SizedBox.expand(),
    );
  }
}

/// Paints [StrataBands].
class StrataBandsPainter extends CustomPainter {
  /// Creates the painter.
  const new({
    required this.depths,
    required this.seamAfter,
    required this.base,
    required this.tone,
    required this.seam,
  });

  /// Relative band depths, top to bottom.
  final List<double> depths;

  /// Index of the band after which the seam is drawn.
  final int seamAfter;

  /// Colour of the top band.
  final Color base;

  /// Colour lower bands step towards.
  final Color tone;

  /// Seam colour (sand).
  final Color seam;

  /// Opacity step between neighbouring bands ("one tone apart").
  static const double toneStep = 0.08;

  /// Seam thickness in logical pixels.
  static const double seamThickness = 2;

  /// Colour of band [index].
  Color bandColor(int index) =>
      Color.alphaBlend(tone.withValues(alpha: toneStep * index), base);

  @override
  void paint(Canvas canvas, Size size) {
    final total = depths.fold<double>(0, (sum, d) => sum + d);
    if (total <= 0 || size.isEmpty) return;
    var top = 0.0;
    double? seamY;
    for (var i = 0; i < depths.length; i++) {
      final height = size.height * depths[i] / total;
      canvas.drawRect(
        Rect.fromLTWH(0, top, size.width, height),
        Paint()..color = bandColor(i),
      );
      top += height;
      if (i == seamAfter) seamY = top;
    }
    if (seamY != null) {
      canvas.drawRect(
        Rect.fromLTWH(0, seamY - seamThickness / 2, size.width, seamThickness),
        Paint()..color = seam,
      );
    }
  }

  @override
  bool shouldRepaint(StrataBandsPainter oldDelegate) =>
      oldDelegate.base != base ||
      oldDelegate.tone != tone ||
      oldDelegate.seam != seam ||
      oldDelegate.seamAfter != seamAfter ||
      !_listEquals(oldDelegate.depths, depths);

  static bool _listEquals(List<double> a, List<double> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }
}
