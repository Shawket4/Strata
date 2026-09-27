import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/edge_drawing.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Geometry of the widget mind map: the core's layout positions scaled for
/// cards, and the content box they live in.
class MindMapLayout {
  /// Lays out [scene] with [cardSize] cards.
  factory of(GraphScene scene, {required Size cardSize}) {
    final bounds = scene.bounds;
    final origin = Offset(
      -bounds.left * spacing + cardSize.width / 2 + margin,
      -bounds.top * spacing + cardSize.height / 2 + margin,
    );
    return MindMapLayout._(
      scene: scene,
      cardSize: cardSize,
      origin: origin,
      size: Size(
        bounds.width * spacing + cardSize.width + margin * 2,
        bounds.height * spacing + cardSize.height + margin * 2,
      ),
    );
  }

  new _({
    required this.scene,
    required this.cardSize,
    required this.origin,
    required this.size,
  });

  /// Layout units → canvas pixels (cards are wider than graph dots).
  static const double spacing = 1.8;

  /// Empty space around the outermost cards.
  static const double margin = 80;

  /// The graph.
  final GraphScene scene;

  /// Card size.
  final Size cardSize;

  /// Canvas position of the layout origin.
  final Offset origin;

  /// Canvas size.
  final Size size;

  /// Canvas centre of node [i].
  Offset centreOf(int i) => scene.positionOf(i) * spacing + origin;

  /// How far from a card's centre its border is along [direction] (unit).
  double borderDistance(Offset direction) {
    final hw = cardSize.width / 2;
    final hh = cardSize.height / 2;
    final dx = direction.dx.abs();
    final dy = direction.dy.abs();
    if (dx < 1e-6) return hh;
    if (dy < 1e-6) return hw;
    return math.min(hw / dx, hh / dy);
  }
}

/// The local mind map canvas (D4 = b): node cards and edge labels as widgets
/// over an edge painter, all inside the caller's `InteractiveViewer`.
class MindMapCanvas extends StatelessWidget {
  /// Creates the canvas.
  const new({
    required this.layout,
    required this.centerId,
    required this.onTapNode,
    required this.onTapEdge,
    super.key,
    this.hiddenEdges = const {},
    this.selectedNodeId,
    this.selectedEdge,
  });

  /// Geometry.
  final MindMapLayout layout;

  /// The focused note.
  final String centerId;

  /// Edge classes hidden by the viewer.
  final Set<EdgeClass> hiddenEdges;

  /// Selected node (ring).
  final String? selectedNodeId;

  /// Selected edge index (highlighted label).
  final int? selectedEdge;

  /// A node card was activated (its index).
  final ValueChanged<int> onTapNode;

  /// An edge label was activated (its index).
  final ValueChanged<int> onTapEdge;

  @override
  Widget build(BuildContext context) {
    final scene = layout.scene;
    final l10n = context.l10n;
    final maps = context.mapsL10n;
    return SizedBox.fromSize(
      size: layout.size,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned.fill(
            child: CustomPaint(
              painter: MindMapEdgePainter(
                layout: layout,
                graph: context.strataGraphColors,
                hiddenEdges: hiddenEdges,
                selectedEdge: selectedEdge,
              ),
            ),
          ),
          for (var e = 0; e < scene.edgeCount; e++)
            if (!hiddenEdges.contains(scene.edgeClasses[e]) &&
                scene.edgeClasses[e] != EdgeClass.bodyLink)
              _EdgeLabel(
                centre:
                    (layout.centreOf(scene.edgeSrc[e]) +
                        layout.centreOf(scene.edgeDst[e])) /
                    2,
                label: l10n.relationTypeLabel(
                  relationTypeOf(scene.edgeClasses[e]),
                ),
                edgeClass: scene.edgeClasses[e],
                confidence: scene.edges[e].by == 'ai'
                    ? scene.edges[e].confidence
                    : null,
                selected: selectedEdge == e,
                semanticLabel: maps.edgeSemantics(
                  type: l10n.relationTypeLabel(
                    relationTypeOf(scene.edgeClasses[e]),
                  ),
                  from: scene.nodes[scene.edgeSrc[e]].title,
                  to: scene.nodes[scene.edgeDst[e]].title,
                ),
                onTap: () => onTapEdge(e),
              ),
          for (var i = 0; i < scene.nodeCount; i++)
            Positioned(
              left: layout.centreOf(i).dx - layout.cardSize.width / 2,
              top: layout.centreOf(i).dy - layout.cardSize.height / 2,
              width: layout.cardSize.width,
              height: layout.cardSize.height,
              child: _NodeCard(
                node: scene.nodes[i],
                kind: scene.kinds[i],
                focus: scene.nodes[i].id == centerId,
                selected: scene.nodes[i].id == selectedNodeId,
                onTap: () => onTapNode(i),
              ),
            ),
        ],
      ),
    );
  }
}

class _NodeCard extends StatelessWidget {
  const new({
    required this.node,
    required this.kind,
    required this.focus,
    required this.selected,
    required this.onTap,
  });

  final GraphNode node;
  final NodeKind kind;
  final bool focus;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final graph = context.strataGraphColors;
    final text = context.strataText;
    final l10n = context.mapsL10n;
    final border = selected || focus
        ? BorderSide(color: graph.selectedRing, width: focus ? 3 : 2)
        : BorderSide(color: colors.border);
    return Semantics(
      button: true,
      selected: selected,
      label: l10n.nodeSemantics(
        kind: context.l10n.nodeKindLabel(kind),
        title: node.title,
      ),
      hint: focus ? null : l10n.recentreOn(title: node.title),
      excludeSemantics: true,
      child: Material(
        color: focus ? colors.accentTint : colors.surface,
        shape: RoundedRectangleBorder(
          borderRadius: StrataRadii.cardRadius,
          side: border,
        ),
        child: InkWell(
          customBorder: const RoundedRectangleBorder(
            borderRadius: StrataRadii.cardRadius,
          ),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s2,
              vertical: StrataSpacing.s1,
            ),
            child: Row(
              children: [
                NodeKindGlyph(kind: kind, decorative: true, size: 18),
                const SizedBox(width: StrataSpacing.s2),
                Expanded(
                  child: Text(
                    node.title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    textAlign: TextAlign.start,
                    style: (focus ? text.bodySmall : text.caption).withWeight(
                      focus ? FontWeight.w700 : FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _EdgeLabel extends StatelessWidget {
  const new({
    required this.centre,
    required this.label,
    required this.edgeClass,
    required this.confidence,
    required this.selected,
    required this.semanticLabel,
    required this.onTap,
  });

  final Offset centre;
  final String label;
  final EdgeClass edgeClass;
  final double? confidence;
  final bool selected;
  final String semanticLabel;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final shared = context.l10n;
    final c = confidence;
    return Positioned(
      left: centre.dx,
      top: centre.dy,
      child: FractionalTranslation(
        translation: const Offset(-0.5, -0.5),
        child: StrataTapTarget(
          semanticLabel: semanticLabel,
          selected: selected,
          onTap: onTap,
          child: Container(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s2,
              vertical: 2,
            ),
            decoration: BoxDecoration(
              color: selected ? colors.accentTint : colors.surface,
              border: Border.all(
                color: selected ? colors.accent : colors.border,
              ),
              borderRadius: StrataRadii.pillRadius,
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                RelationLineSample(
                  type: relationTypeOf(edgeClass),
                  width: 16,
                  height: 10,
                ),
                const SizedBox(width: StrataSpacing.s1),
                Text(label, style: text.caption.copyWith(color: colors.text2)),
                if (c != null) ...[
                  const SizedBox(width: StrataSpacing.s1),
                  Text(
                    RelationChip.aiTagText(shared.aiTag, c),
                    style: text.caption
                        .withWeight(FontWeight.w600)
                        .copyWith(color: colors.infoText),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Paints the mind map's edges under the cards, in each relation's style,
/// ending at the target card's border.
class MindMapEdgePainter extends CustomPainter {
  /// Creates the painter.
  const new({
    required this.layout,
    required this.graph,
    required this.hiddenEdges,
    this.selectedEdge,
  });

  /// Geometry.
  final MindMapLayout layout;

  /// Colours.
  final StrataGraphColors graph;

  /// Hidden edge classes.
  final Set<EdgeClass> hiddenEdges;

  /// Selected edge (drawn wider).
  final int? selectedEdge;

  @override
  void paint(Canvas canvas, Size size) {
    final scene = layout.scene;
    for (var e = 0; e < scene.edgeCount; e++) {
      final cls = scene.edgeClasses[e];
      if (hiddenEdges.contains(cls)) continue;
      final a = layout.centreOf(scene.edgeSrc[e]);
      final b = layout.centreOf(scene.edgeDst[e]);
      final delta = b - a;
      if (delta.distance < 1) continue;
      final dir = delta / delta.distance;
      final color = graph.relation(
        relationTypeOf(cls),
        mentionOf: scene.kinds[scene.edgeDst[e]],
      );
      drawStyledEdge(
        canvas,
        a,
        b,
        lineStyleOf(cls),
        color,
        trimStart: layout.borderDistance(dir),
        trimEnd: layout.borderDistance(dir) + 3,
        widthScale: e == selectedEdge ? 2 : 1,
        squareEnd: cls == EdgeClass.custody,
      );
    }
  }

  @override
  bool shouldRepaint(MindMapEdgePainter oldDelegate) =>
      !identical(oldDelegate.layout, layout) ||
      oldDelegate.graph != graph ||
      oldDelegate.hiddenEdges != hiddenEdges ||
      oldDelegate.selectedEdge != selectedEdge;
}
