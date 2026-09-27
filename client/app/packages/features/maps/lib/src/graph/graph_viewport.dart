import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:strata_maps/src/graph/graph_camera.dart';
import 'package:strata_maps/src/graph/graph_painter.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';

/// A painted graph with pan (drag), zoom (pinch, wheel, `+` / `-` / `0`),
/// tap and hover hit-testing. Painting is one [GraphPainter]; the camera
/// lives in [view] so panning repaints without rebuilding widgets.
class GraphViewport extends StatefulWidget {
  /// Creates the viewport.
  const new({
    required this.scene,
    required this.view,
    required this.semanticLabel,
    super.key,
    this.options = const GraphPaintOptions(),
    this.interactive = true,
    this.clusterRegions = true,
    this.onTapNode,
    this.onHoverNode,
    this.onEscape,
    this.shortcuts = const {},
  });

  /// The graph.
  final GraphScene scene;

  /// Camera.
  final GraphViewController view;

  /// Accessibility label of the canvas.
  final String semanticLabel;

  /// Visibility, selection and dimming.
  final GraphPaintOptions options;

  /// Whether pan, zoom and hit-testing are on.
  final bool interactive;

  /// Whether cluster regions are drawn.
  final bool clusterRegions;

  /// A tap on a node (its index) or on empty canvas (`null`).
  final ValueChanged<int?>? onTapNode;

  /// The hovered node (index and pointer position) or `null`.
  final void Function(int? node, Offset position)? onHoverNode;

  /// Escape pressed while the canvas has focus.
  final VoidCallback? onEscape;

  /// More keyboard shortcuts active while the canvas has focus.
  final Map<ShortcutActivator, VoidCallback> shortcuts;

  @override
  State<GraphViewport> createState() => _GraphViewportState();
}

class _GraphViewportState extends State<GraphViewport> {
  final GraphRenderCache _cache = GraphRenderCache();
  final FocusNode _focus = FocusNode(debugLabel: 'graph');
  GraphCamera? _gestureStart;
  Offset _gestureFocal = Offset.zero;
  int? _hovered;

  @override
  void dispose() {
    _cache.dispose();
    _focus.dispose();
    super.dispose();
  }

  int? _hit(Offset local) {
    final camera = widget.view.camera;
    return widget.scene.hitTest(
      camera.toWorld(local),
      radiusFactor: camera.nodeScale / camera.scale,
      slop: 6 / camera.scale,
    );
  }

  void _onPointerSignal(PointerSignalEvent event) {
    if (event is PointerScrollEvent) {
      final factor = math.exp(-event.scrollDelta.dy / 300);
      widget.view.camera = widget.view.camera.zoomed(
        factor,
        event.localPosition,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final size = constraints.biggest;
        widget.view.attach(widget.scene.bounds, size);
        final painter = CustomPaint(
          size: size,
          isComplex: true,
          painter: GraphPainter(
            scene: widget.scene,
            view: widget.view,
            palette: GraphPalette.of(context),
            cache: _cache,
            textDirection: Directionality.of(context),
            options: widget.options,
            clusterRegions: widget.clusterRegions,
          ),
        );
        final canvas = Semantics(
          label: widget.semanticLabel,
          image: true,
          child: RepaintBoundary(child: painter),
        );
        if (!widget.interactive) return canvas;
        return CallbackShortcuts(
          bindings: {
            const SingleActivator(LogicalKeyboardKey.equal): () =>
                widget.view.zoomBy(1.25),
            const SingleActivator(LogicalKeyboardKey.add): () =>
                widget.view.zoomBy(1.25),
            const SingleActivator(LogicalKeyboardKey.numpadAdd): () =>
                widget.view.zoomBy(1.25),
            const SingleActivator(LogicalKeyboardKey.minus): () =>
                widget.view.zoomBy(0.8),
            const SingleActivator(LogicalKeyboardKey.numpadSubtract): () =>
                widget.view.zoomBy(0.8),
            const SingleActivator(LogicalKeyboardKey.digit0): widget.view.fit,
            const SingleActivator(LogicalKeyboardKey.escape): () =>
                widget.onEscape?.call(),
            ...widget.shortcuts,
          },
          child: Focus(
            focusNode: _focus,
            child: Listener(
              onPointerSignal: _onPointerSignal,
              child: MouseRegion(
                cursor: _hovered == null
                    ? SystemMouseCursors.grab
                    : SystemMouseCursors.click,
                onHover: (event) {
                  final hit = _hit(event.localPosition);
                  if (hit != _hovered) setState(() => _hovered = hit);
                  widget.onHoverNode?.call(hit, event.localPosition);
                },
                onExit: (event) {
                  if (_hovered != null) setState(() => _hovered = null);
                  widget.onHoverNode?.call(null, event.localPosition);
                },
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTapUp: (details) {
                    _focus.requestFocus();
                    widget.onTapNode?.call(_hit(details.localPosition));
                  },
                  onScaleStart: (details) {
                    _gestureStart = widget.view.camera;
                    _gestureFocal = details.localFocalPoint;
                  },
                  onScaleUpdate: (details) {
                    final start = _gestureStart;
                    if (start == null) return;
                    widget.view.camera = start
                        .zoomed(details.scale, _gestureFocal)
                        .panned(details.localFocalPoint - _gestureFocal);
                  },
                  onScaleEnd: (_) => _gestureStart = null,
                  child: canvas,
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
