import 'dart:math' as math;
import 'dart:ui';

import 'package:flutter/foundation.dart';

/// Label density by zoom (GraphLanguage "Labels by zoom").
enum ZoomBand {
  /// Below 40 %: cluster labels only, thin edges, no arrows.
  far,

  /// 40–100 %: adds labels for high-degree nodes.
  mid,

  /// 100 % and above: all labels, arrowheads, × marks, tags.
  near;

  /// The band of a zoom [scale].
  static ZoomBand of(double scale) {
    if (scale < 0.4) return ZoomBand.far;
    if (scale < 1) return ZoomBand.mid;
    return ZoomBand.near;
  }
}

/// The view transform of a painted graph: `screen = world × scale + offset`.
/// Pure view state (pan and zoom), owned by the widget.
@immutable
class GraphCamera {
  /// Creates a camera.
  const new({this.scale = 1, this.offset = Offset.zero});

  /// A camera showing [bounds] (world units) centred in [viewport] with
  /// [padding] screen pixels around it, clamped to [minScale]–[maxScale].
  factory fit(
    Rect bounds,
    Size viewport, {
    double padding = 48,
    double maxScale = 1.5,
  }) {
    final w = math.max(viewport.width - padding * 2, 1);
    final h = math.max(viewport.height - padding * 2, 1);
    final scale = bounds.width <= 0 && bounds.height <= 0
        ? 1.0
        : math
              .min(
                bounds.width <= 0 ? maxScale : w / bounds.width,
                bounds.height <= 0 ? maxScale : h / bounds.height,
              )
              .clamp(minScale, maxScale);
    final centre = bounds.center;
    return GraphCamera(
      scale: scale,
      offset: Offset(
        viewport.width / 2 - centre.dx * scale,
        viewport.height / 2 - centre.dy * scale,
      ),
    );
  }

  /// Smallest zoom.
  static const double minScale = 0.05;

  /// Largest zoom.
  static const double maxScale = 4;

  /// Zoom factor.
  final double scale;

  /// Screen position of the world origin.
  final Offset offset;

  /// The zoom band (label density).
  ZoomBand get band => ZoomBand.of(scale);

  /// Screen radius multiplier for nodes: nodes shrink less than the world so
  /// they stay visible when zoomed out and don't balloon when zoomed in.
  double get nodeScale => scale.clamp(0.35, 1.6);

  /// World position of a screen point.
  Offset toWorld(Offset screen) => (screen - offset) / scale;

  /// Screen position of a world point.
  Offset toScreen(Offset world) => world * scale + offset;

  /// Pans by a screen [delta].
  GraphCamera panned(Offset delta) =>
      GraphCamera(scale: scale, offset: offset + delta);

  /// Zooms by [factor] keeping the screen point [focal] fixed.
  GraphCamera zoomed(double factor, Offset focal) {
    final next = (scale * factor).clamp(minScale, maxScale);
    final world = toWorld(focal);
    return GraphCamera(scale: next, offset: focal - world * next);
  }

  /// Centres the world point [world] in [viewport], keeping the zoom (or
  /// using at least [minZoom]).
  GraphCamera centredOn(Offset world, Size viewport, {double minZoom = 0}) {
    final next = math.max(scale, minZoom);
    return GraphCamera(
      scale: next,
      offset: Offset(viewport.width / 2, viewport.height / 2) - world * next,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is GraphCamera && other.scale == scale && other.offset == offset;

  @override
  int get hashCode => Object.hash(scale, offset);
}
