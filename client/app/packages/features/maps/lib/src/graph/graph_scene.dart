import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui';

import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The render-ready form of a core graph view (global map, local mind map,
/// mini graph): node positions and styles in flat typed arrays, edges as
/// node-index pairs, and a uniform grid for hit-testing.
///
/// Built once per view-model value; the painter reads it every frame. Nothing
/// here decides what the graph contains: positions, degrees, clusters and
/// edges are the core's (D3 = a); this only indexes them for drawing.
class GraphScene {
  GraphScene._({
    required this.nodes,
    required this.edges,
    required this.index,
    required this.xs,
    required this.ys,
    required this.radii,
    required this.kinds,
    required this.edgeSrc,
    required this.edgeDst,
    required this.edgeClasses,
    required this.bounds,
    required Map<int, List<int>> grid,
  }) : _grid = grid;

  /// Indexes [nodes] and [edges] (edges whose ends are not in [nodes] are not
  /// drawn).
  factory GraphScene.from(List<GraphNode> nodes, List<GraphEdge> edges) {
    final n = nodes.length;
    final index = <String, int>{};
    final xs = Float32List(n);
    final ys = Float32List(n);
    final radii = Float32List(n);
    final kinds = List<NodeKind>.filled(n, NodeKind.note);
    var minX = double.infinity;
    var minY = double.infinity;
    var maxX = double.negativeInfinity;
    var maxY = double.negativeInfinity;
    final grid = <int, List<int>>{};
    for (var i = 0; i < n; i++) {
      final node = nodes[i];
      index[node.id] = i;
      xs[i] = node.x;
      ys[i] = node.y;
      final kind = nodeKindOf(node.kind);
      kinds[i] = kind;
      radii[i] = radiusOf(kind, node.degree);
      minX = math.min(minX, node.x);
      minY = math.min(minY, node.y);
      maxX = math.max(maxX, node.x);
      maxY = math.max(maxY, node.y);
      grid.putIfAbsent(_cellKey(node.x, node.y), () => <int>[]).add(i);
    }
    final src = <int>[];
    final dst = <int>[];
    final classes = <EdgeClass>[];
    final kept = <GraphEdge>[];
    for (final edge in edges) {
      final a = index[edge.src];
      final b = index[edge.dst];
      if (a == null || b == null) continue;
      src.add(a);
      dst.add(b);
      classes.add(edgeClassOf(edge.kind));
      kept.add(edge);
    }
    return GraphScene._(
      nodes: nodes,
      edges: kept,
      index: index,
      xs: xs,
      ys: ys,
      radii: radii,
      kinds: kinds,
      edgeSrc: Int32List.fromList(src),
      edgeDst: Int32List.fromList(dst),
      edgeClasses: classes,
      bounds: n == 0
          ? Rect.zero
          : Rect.fromLTRB(minX, minY, maxX, maxY),
      grid: grid,
    );
  }

  /// World size of one hit-testing grid cell.
  static const double cellSize = 48;

  /// The largest node radius (world units at 100 %).
  static const double maxRadius = 12;

  /// Node radius per kind (GraphLanguage: notes grow with degree, 4–12).
  static double radiusOf(NodeKind kind, int degree) => switch (kind) {
    NodeKind.note => 4 + math.min(degree, 16) / 2,
    _ => 7,
  };

  static int _cellKey(double x, double y) =>
      _cellKeyOf((x / cellSize).floor(), (y / cellSize).floor());

  static int _cellKeyOf(int cx, int cy) => cx * 73856093 ^ cy * 19349663;

  /// The core's nodes, in view order.
  final List<GraphNode> nodes;

  /// The drawable edges (both ends known), in view order.
  final List<GraphEdge> edges;

  /// Node index by ID.
  final Map<String, int> index;

  /// World x per node.
  final Float32List xs;

  /// World y per node.
  final Float32List ys;

  /// Radius per node at 100 %.
  final Float32List radii;

  /// Kind per node.
  final List<NodeKind> kinds;

  /// Source node index per edge.
  final Int32List edgeSrc;

  /// Target node index per edge.
  final Int32List edgeDst;

  /// Class per edge.
  final List<EdgeClass> edgeClasses;

  /// Bounding box of every node position (world units).
  final Rect bounds;

  final Map<int, List<int>> _grid;

  /// Number of nodes.
  int get nodeCount => xs.length;

  /// Number of drawable edges.
  int get edgeCount => edgeSrc.length;

  /// World position of node [i].
  Offset positionOf(int i) => Offset(xs[i], ys[i]);

  /// The node under [world] (world units): within its radius × [radiusFactor]
  /// plus [slop] (both world units), nearest first; `null` when none.
  int? hitTest(Offset world, {double radiusFactor = 1, double slop = 4}) {
    final cx = (world.dx / cellSize).floor();
    final cy = (world.dy / cellSize).floor();
    final reach = ((maxRadius * radiusFactor + slop) / cellSize).ceil();
    int? best;
    var bestDistance = double.infinity;
    for (var dx = -reach; dx <= reach; dx++) {
      for (var dy = -reach; dy <= reach; dy++) {
        final cell = _grid[_cellKeyOf(cx + dx, cy + dy)];
        if (cell == null) continue;
        for (final i in cell) {
          final ddx = xs[i] - world.dx;
          final ddy = ys[i] - world.dy;
          final d = math.sqrt(ddx * ddx + ddy * ddy);
          if (d <= radii[i] * radiusFactor + slop && d < bestDistance) {
            bestDistance = d;
            best = i;
          }
        }
      }
    }
    return best;
  }
}
