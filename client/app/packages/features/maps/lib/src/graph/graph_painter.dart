import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:strata_maps/src/graph/graph_camera.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_ui/strata_ui.dart';

/// What the viewer chose to see (ephemeral widget state, applied while
/// painting as layer visibility and dimming; the graph itself is the core's).
@immutable
class GraphPaintOptions {
  /// Creates paint options.
  const new({
    this.hiddenEdges = const {},
    this.hiddenKinds = const {},
    this.focusCluster,
    this.selected,
    this.highlight,
    this.hovered,
    this.labels = true,
  });

  /// Edge classes not drawn.
  final Set<EdgeClass> hiddenEdges;

  /// Node kinds not drawn (with their edges).
  final Set<NodeKind> hiddenKinds;

  /// Cluster in focus: nodes and edges outside it are dimmed.
  final String? focusCluster;

  /// Selected node index (3 px ring, label in 600).
  final int? selected;

  /// Node indices kept at full strength while something is selected (the
  /// neighbourhood the core returned); everything else is dimmed to 30 %.
  final Set<int>? highlight;

  /// Hovered node index (tint halo, label shown).
  final int? hovered;

  /// Whether node labels are drawn.
  final bool labels;
}

/// Colours and text styles the painter uses (from the Strata theme).
@immutable
class GraphPalette {
  /// Creates a palette.
  const new({
    required this.graph,
    required this.halo,
    required this.hoverTint,
    required this.labelStyle,
    required this.labelColor,
  });

  /// Reads the palette from the ambient Strata theme.
  factory of(BuildContext context) {
    final colors = context.strataColors;
    return GraphPalette(
      graph: context.strataGraphColors,
      halo: colors.background,
      hoverTint: colors.accentTint,
      labelStyle: _withoutColor(context.strataText.caption),
      labelColor: colors.text,
    );
  }

  /// Node and relation colours.
  final StrataGraphColors graph;

  /// Label halo colour (mist).
  final Color halo;

  /// Hover halo colour.
  final Color hoverTint;

  /// Node label style (Cairo 12), without a colour.
  final TextStyle labelStyle;

  /// Node label colour.
  final Color labelColor;

  static TextStyle _withoutColor(TextStyle s) => TextStyle(
    fontFamily: s.fontFamily,
    fontFamilyFallback: s.fontFamilyFallback,
    fontSize: s.fontSize,
    fontWeight: s.fontWeight,
    fontVariations: s.fontVariations,
    height: s.height,
    letterSpacing: s.letterSpacing,
  );

  @override
  bool operator ==(Object other) =>
      other is GraphPalette &&
      other.graph == graph &&
      other.halo == halo &&
      other.hoverTint == hoverTint &&
      other.labelStyle == labelStyle &&
      other.labelColor == labelColor;

  @override
  int get hashCode =>
      Object.hash(graph, halo, hoverTint, labelStyle, labelColor);
}

/// Reusable scratch buffers and laid-out labels, kept by the widget between
/// frames so painting allocates little.
class GraphRenderCache {
  final Map<int, _Batch> _lines = {};
  final Map<int, _Batch> _points = {};
  final Map<int, _Label> _labels = {};
  GraphScene? _scene;
  GraphPalette? _palette;
  TextDirection? _direction;

  void _prepare(GraphScene scene, GraphPalette palette, TextDirection dir) {
    if (!identical(_scene, scene) || _palette != palette || _direction != dir) {
      for (final label in _labels.values) {
        label.dispose();
      }
      _labels.clear();
      _scene = scene;
      _palette = palette;
      _direction = dir;
    }
    for (final batch in _lines.values) {
      batch.clear();
    }
    for (final batch in _points.values) {
      batch.clear();
    }
  }

  /// Releases the laid-out labels.
  void dispose() {
    for (final label in _labels.values) {
      label.dispose();
    }
    _labels.clear();
  }
}

class _Batch {
  Float32List _data = Float32List(256);
  int _length = 0;
  Paint? paint;

  bool get isEmpty => _length == 0;

  void clear() => _length = 0;

  void add2(double x, double y) {
    if (_length + 2 > _data.length) _grow();
    _data[_length] = x;
    _data[_length + 1] = y;
    _length += 2;
  }

  void add4(double x1, double y1, double x2, double y2) {
    if (_length + 4 > _data.length) _grow();
    _data[_length] = x1;
    _data[_length + 1] = y1;
    _data[_length + 2] = x2;
    _data[_length + 3] = y2;
    _length += 4;
  }

  void _grow() {
    _data = Float32List(_data.length * 2)..setRange(0, _length, _data);
  }

  Float32List get view => Float32List.sublistView(_data, 0, _length);
}

class _Label {
  new(this.fill, this.stroke);

  final TextPainter fill;
  final TextPainter stroke;

  void dispose() {
    fill.dispose();
    stroke.dispose();
  }
}

/// Paints a whole graph (global map, mini graph) on one canvas: cluster
/// regions, edges batched by style, nodes by kind, selection and labels by
/// zoom band (D3 = a; GraphLanguage). Built for 10k nodes / 40k edges:
/// per-frame work is linear with culling, edges and note discs are drawn as
/// raw point batches (one draw call per style), labels are cached.
class GraphPainter extends CustomPainter {
  /// Creates the painter.
  new({
    required this.scene,
    required this.view,
    required this.palette,
    required this.cache,
    required this.textDirection,
    this.options = const GraphPaintOptions(),
    this.clusterRegions = true,
  }) : super(repaint: view);

  /// The graph.
  final GraphScene scene;

  /// Pan and zoom (repaints when it changes).
  final GraphViewController view;

  /// Colours.
  final GraphPalette palette;

  /// Scratch buffers.
  final GraphRenderCache cache;

  /// Direction for labels.
  final TextDirection textDirection;

  /// Visibility, selection and dimming.
  final GraphPaintOptions options;

  /// Whether cluster regions are drawn.
  final bool clusterRegions;

  /// Opacity of dimmed (unrelated) nodes and edges.
  static const double dimOpacity = 0.3;

  /// World radius of the soft region drawn around every clustered node.
  static const double clusterHalo = 60;

  /// Most labels drawn in one frame.
  static const int labelBudget = 300;

  /// Degree from which a node is a "hub" labelled at mid zoom.
  static const int hubDegree = 6;

  @override
  void paint(Canvas canvas, Size size) {
    cache._prepare(scene, palette, textDirection);
    view.attach(scene.bounds, size);
    final camera = view.camera;
    final n = scene.nodeCount;
    if (n == 0) return;
    final s = camera.scale;
    final ox = camera.offset.dx;
    final oy = camera.offset.dy;
    final band = camera.band;
    final nodeScale = camera.nodeScale;
    final margin = GraphScene.maxRadius * nodeScale + 8;
    final minX = -margin;
    final minY = -margin;
    final maxX = size.width + margin;
    final maxY = size.height + margin;
    final clip = Rect.fromLTRB(minX, minY, maxX, maxY);
    final xs = scene.xs;
    final ys = scene.ys;
    final kinds = scene.kinds;
    final nodes = scene.nodes;
    final hiddenKinds = options.hiddenKinds;
    final focus = options.focusCluster;
    final highlight = options.highlight;
    final selected = options.selected;

    // Screen positions and per-node state, computed once per frame.
    final sx = Float32List(n);
    final sy = Float32List(n);
    final visible = Uint8List(n);
    final dim = Uint8List(n);
    for (var i = 0; i < n; i++) {
      final x = xs[i] * s + ox;
      final y = ys[i] * s + oy;
      sx[i] = x;
      sy[i] = y;
      if (hiddenKinds.contains(kinds[i])) continue;
      visible[i] = 1;
      if ((highlight != null && !highlight.contains(i)) ||
          (focus != null && nodes[i].clusterId != focus)) {
        dim[i] = 1;
      }
    }

    // Cluster regions: the union of soft discs around clustered nodes, filled
    // once at the region colour (tide 7 %).
    if (clusterRegions) {
      final halo = _pointBatch(-1);
      for (var i = 0; i < n; i++) {
        if (visible[i] == 0 || nodes[i].clusterId == null) continue;
        if (focus != null && nodes[i].clusterId != focus) continue;
        final x = sx[i];
        final y = sy[i];
        final r = clusterHalo * s;
        if (x < -r || y < -r || x > size.width + r || y > size.height + r) {
          continue;
        }
        halo.add2(x, y);
      }
      if (!halo.isEmpty) {
        final fill = palette.graph.clusterFill;
        canvas
          ..saveLayer(
            Offset.zero & size,
            Paint()..color = Color.fromRGBO(0, 0, 0, fill.a),
          )
          ..drawRawPoints(
            ui.PointMode.points,
            halo.view,
            Paint()
              ..color = fill.withValues(alpha: 1)
              ..strokeWidth = clusterHalo * 2 * s
              ..strokeCap = StrokeCap.round,
          )
          ..restore();
      }
    }

    // Edges, batched by (class, dimmed, mention target kind).
    final hiddenEdges = options.hiddenEdges;
    final src = scene.edgeSrc;
    final dst = scene.edgeDst;
    final classes = scene.edgeClasses;
    final details = band == ZoomBand.near;
    final styled = band != ZoomBand.far;
    for (var e = 0; e < src.length; e++) {
      final cls = classes[e];
      if (hiddenEdges.contains(cls)) continue;
      final a = src[e];
      final b = dst[e];
      if (visible[a] == 0 || visible[b] == 0) continue;
      var x1 = sx[a];
      var y1 = sy[a];
      var x2 = sx[b];
      var y2 = sy[b];
      if ((x1 < minX && x2 < minX) ||
          (y1 < minY && y2 < minY) ||
          (x1 > maxX && x2 > maxX) ||
          (y1 > maxY && y2 > maxY)) {
        continue;
      }
      final dimmed =
          (highlight != null && a != selected && b != selected) ||
          dim[a] == 1 ||
          dim[b] == 1;
      final mentionKind = cls == EdgeClass.mention ? kinds[b].index : 0;
      final key = cls.index * 32 + (dimmed ? 16 : 0) + mentionKind;
      final batch = _lineBatch(key);
      final style = lineStyleOf(cls);
      final dx = x2 - x1;
      final dy = y2 - y1;
      final length = math.sqrt(dx * dx + dy * dy);
      if (length < 1) continue;
      final ux = dx / length;
      final uy = dy / length;
      if (details) {
        final trimA = scene.radii[a] * nodeScale + 3;
        final trimB = scene.radii[b] * nodeScale + 3;
        if (length <= trimA + trimB + 2) continue;
        x1 += ux * trimA;
        y1 += uy * trimA;
        x2 -= ux * trimB;
        y2 -= uy * trimB;
      }
      final pattern = styled ? style.dashPattern : null;
      if (styled && style.doubleLine) {
        final nx = -uy * 1.75;
        final ny = ux * 1.75;
        _segment(batch, x1 + nx, y1 + ny, x2 + nx, y2 + ny, pattern, clip);
        _segment(batch, x1 - nx, y1 - ny, x2 - nx, y2 - ny, pattern, clip);
      } else {
        _segment(batch, x1, y1, x2, y2, pattern, clip);
      }
      if (details) {
        final head = _lineBatch(key + 1000000);
        if (style.arrow) {
          const h = 6.0;
          final bx = x2 - ux * h;
          final by = y2 - uy * h;
          head
            ..add4(bx - uy * h * 0.6, by + ux * h * 0.6, x2, y2)
            ..add4(bx + uy * h * 0.6, by - ux * h * 0.6, x2, y2);
        }
        if (style.midCross) {
          final mx = (x1 + x2) / 2;
          final my = (y1 + y2) / 2;
          const r = 3.5 / math.sqrt2;
          final px = (ux - uy) * r;
          final py = (uy + ux) * r;
          final qx = (ux + uy) * r;
          final qy = (uy - ux) * r;
          head
            ..add4(mx - px, my - py, mx + px, my + py)
            ..add4(mx - qx, my - qy, mx + qx, my + qy);
        }
        if (cls == EdgeClass.custody) {
          head
            ..add4(x2 - 2.5, y2 - 2.5, x2 + 2.5, y2 - 2.5)
            ..add4(x2 + 2.5, y2 - 2.5, x2 + 2.5, y2 + 2.5)
            ..add4(x2 + 2.5, y2 + 2.5, x2 - 2.5, y2 + 2.5)
            ..add4(x2 - 2.5, y2 + 2.5, x2 - 2.5, y2 - 2.5);
        }
      }
    }
    // Dimmed first, so related edges draw on top.
    final lineKeys = cache._lines.keys.toList()
      ..sort((p, q) => ((q >> 4) & 1).compareTo((p >> 4) & 1));
    for (final key in lineKeys) {
      final batch = cache._lines[key]!;
      if (batch.isEmpty) continue;
      final base = key % 1000000;
      final cls = EdgeClass.values[base ~/ 32];
      final dimmed = (base >> 4) & 1 == 1;
      final mentionOf = NodeKind.values[base % 16];
      final style = lineStyleOf(cls);
      final color = palette.graph.relation(
        relationTypeOf(cls),
        mentionOf: mentionOf,
      );
      final width = !styled ? 0.75 : style.strokeWidth;
      canvas.drawRawPoints(
        ui.PointMode.lines,
        batch.view,
        Paint()
          ..color = color.withValues(
            alpha: color.a * style.opacity * (dimmed ? dimOpacity : 1),
          )
          ..strokeWidth = key >= 1000000 ? math.max(1.5, width) : width
          ..strokeCap = key >= 1000000 ? StrokeCap.round : StrokeCap.butt,
      );
    }

    // Hover halo.
    final hovered = options.hovered;
    if (hovered != null && hovered < n && visible[hovered] == 1) {
      canvas.drawCircle(
        Offset(sx[hovered], sy[hovered]),
        scene.radii[hovered] * nodeScale + 7,
        Paint()..color = palette.hoverTint,
      );
    }

    // Nodes: dimmed pass, then full-strength pass.
    final tiny = band == ZoomBand.far;
    for (final pass in const [1, 0]) {
      final alpha = pass == 1 ? dimOpacity : 1.0;
      for (var i = 0; i < n; i++) {
        if (visible[i] == 0 || dim[i] != pass) continue;
        final x = sx[i];
        final y = sy[i];
        if (x < minX || y < minY || x > maxX || y > maxY) continue;
        final r = scene.radii[i] * nodeScale;
        final kind = kinds[i];
        if (kind == NodeKind.note || tiny) {
          final bucket = (r * 2).round();
          _pointBatch(kind.index * 1000 + pass * 100 + bucket).add2(x, y);
        } else {
          final colors = palette.graph.node(kind);
          canvas
            ..save()
            ..translate(x - r - 1, y - r - 1);
          NodeKindPainter(
            shape: nodeShapeOf(kind),
            colors: pass == 1 ? _faded(colors, alpha) : colors,
          ).paint(canvas, Size.square(r * 2 + 2));
          canvas.restore();
        }
      }
      for (final entry in cache._points.entries) {
        final key = entry.key;
        if (key < 0 || (key % 1000) ~/ 100 != pass) continue;
        final batch = entry.value;
        if (batch.isEmpty) continue;
        final kind = NodeKind.values[key ~/ 1000];
        final colors = palette.graph.node(kind);
        final color = colors.fill ?? colors.stroke;
        canvas.drawRawPoints(
          ui.PointMode.points,
          batch.view,
          Paint()
            ..color = color.withValues(alpha: color.a * alpha)
            ..strokeWidth = (key % 100).toDouble()
            ..strokeCap = StrokeCap.round,
        );
        batch.clear();
      }
    }

    // Selection ring.
    if (selected != null && selected < n && visible[selected] == 1) {
      canvas.drawCircle(
        Offset(sx[selected], sy[selected]),
        scene.radii[selected] * nodeScale + 4,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 3
          ..color = palette.graph.selectedRing,
      );
    }

    // Labels by zoom band.
    if (!options.labels) return;
    var drawn = 0;
    void label(int i) {
      if (drawn >= labelBudget) return;
      final x = sx[i];
      final y = sy[i];
      if (x < minX || y < minY || x > maxX || y > maxY) return;
      drawn++;
      final painted = _labelFor(i, bold: i == selected);
      final top = y + scene.radii[i] * nodeScale + 3;
      final offset = Offset(x - painted.fill.width / 2, top);
      painted.stroke.paint(canvas, offset);
      painted.fill.paint(canvas, offset);
    }

    if (band != ZoomBand.far) {
      for (var i = 0; i < n; i++) {
        if (visible[i] == 0 || dim[i] == 1 || i == selected || i == hovered) {
          continue;
        }
        if (band == ZoomBand.mid && nodes[i].degree < hubDegree) continue;
        label(i);
      }
    }
    if (hovered != null && hovered < n && visible[hovered] == 1) {
      label(hovered);
    }
    if (selected != null && selected < n && visible[selected] == 1) {
      label(selected);
    }
  }

  static NodeKindColors _faded(NodeKindColors c, double alpha) =>
      NodeKindColors(
        fill: c.fill?.withValues(alpha: c.fill!.a * alpha),
        stroke: c.stroke.withValues(alpha: c.stroke.a * alpha),
      );

  /// Adds the part of segment (x1,y1)–(x2,y2) inside [clip] to [batch],
  /// dashed with [pattern] (dash phase anchored at the segment start, so
  /// dashes don't crawl while panning).
  static void _segment(
    _Batch batch,
    double x1,
    double y1,
    double x2,
    double y2,
    List<double>? pattern,
    Rect clip,
  ) {
    final dx = x2 - x1;
    final dy = y2 - y1;
    // Liang–Barsky clipping against the viewport.
    var t0 = 0.0;
    var t1 = 1.0;
    bool edge(double p, double q) {
      if (p == 0) return q >= 0;
      final r = q / p;
      if (p < 0) {
        if (r > t1) return false;
        if (r > t0) t0 = r;
      } else {
        if (r < t0) return false;
        if (r < t1) t1 = r;
      }
      return true;
    }

    if (!edge(-dx, x1 - clip.left) ||
        !edge(dx, clip.right - x1) ||
        !edge(-dy, y1 - clip.top) ||
        !edge(dy, clip.bottom - y1)) {
      return;
    }
    if (pattern == null) {
      batch.add4(x1 + dx * t0, y1 + dy * t0, x1 + dx * t1, y1 + dy * t1);
      return;
    }
    final length = math.sqrt(dx * dx + dy * dy);
    if (length == 0) return;
    final ux = dx / length;
    final uy = dy / length;
    var period = 0.0;
    for (final p in pattern) {
      period += p;
    }
    final from = t0 * length;
    final to = t1 * length;
    var d = (from / period).floor() * period;
    var k = 0;
    while (d < to) {
      final seg = pattern[k % pattern.length];
      if (k.isEven && d + seg > from) {
        final a = math.max(d, from);
        final b = math.min(d + seg, to);
        batch.add4(x1 + ux * a, y1 + uy * a, x1 + ux * b, y1 + uy * b);
      }
      d += seg;
      k++;
    }
  }

  _Batch _lineBatch(int key) => cache._lines.putIfAbsent(key, _Batch.new);

  _Batch _pointBatch(int key) => cache._points.putIfAbsent(key, _Batch.new);

  _Label _labelFor(int i, {required bool bold}) {
    final key = bold ? -i - 1 : i;
    return cache._labels.putIfAbsent(key, () {
      final base = bold
          ? palette.labelStyle.copyWith(
              fontWeight: FontWeight.w600,
              fontVariations: const [ui.FontVariation('wght', 600)],
            )
          : palette.labelStyle;
      final text = scene.nodes[i].title;
      TextPainter make(TextStyle style) => TextPainter(
        text: TextSpan(text: text, style: style),
        textDirection: textDirection,
        maxLines: 1,
        ellipsis: '…',
      )..layout(maxWidth: 180);
      return _Label(
        make(base.copyWith(color: palette.labelColor)),
        make(
          base.copyWith(
            foreground: Paint()
              ..style = PaintingStyle.stroke
              ..strokeWidth = 3
              ..strokeJoin = StrokeJoin.round
              ..color = palette.halo,
          ),
        ),
      );
    });
  }

  @override
  bool shouldRepaint(GraphPainter oldDelegate) =>
      !identical(oldDelegate.scene, scene) ||
      !identical(oldDelegate.view, view) ||
      oldDelegate.palette != palette ||
      oldDelegate.textDirection != textDirection ||
      !identical(oldDelegate.options, options) ||
      oldDelegate.clusterRegions != clusterRegions;
}
