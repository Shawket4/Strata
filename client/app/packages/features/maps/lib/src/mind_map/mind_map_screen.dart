import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/src/common/async_states.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_maps/src/mind_map/edge_details.dart';
import 'package:strata_maps/src/mind_map/mind_map_canvas.dart';
import 'package:strata_maps/src/mind_map/node_aside.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The local mind map of a note (PLAN §11 screen 6, D4 = b): the focused note
/// in the centre with its neighbourhood at the core's radial positions, as
/// widget cards in an [InteractiveViewer] over an edge painter; edge labels by
/// type, tap to recentre, edge sheet / card with Retype / Reject, depth
/// control. Compact: its own top bar and an edge bottom sheet; medium and
/// expanded: toolbar, edge-type filters, a hover card and (expanded) the
/// selected node's panel.
class MindMapScreen extends StatelessWidget {
  /// Creates the mind map of [noteId].
  const new(this.noteId, {super.key, this.onOpenNote, this.onBack});

  /// The focused note.
  final String noteId;

  /// Opens a note (its ID).
  final ValueChanged<String>? onOpenNote;

  /// Back to the note (compact top bar); hidden when `null`.
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) => MapsLocalizationScope(
    child: _MindMap(noteId: noteId, onOpenNote: onOpenNote, onBack: onBack),
  );
}

class _MindMap extends HookConsumerWidget {
  const new({
    required this.noteId,
    required this.onOpenNote,
    required this.onBack,
  });

  final String noteId;
  final ValueChanged<String>? onOpenNote;
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final sizeClass = SizeClass.of(context);
    final compact = sizeClass == SizeClass.compact;
    final center = useState(noteId);
    useValueChanged<String, void>(noteId, (_, _) => center.value = noteId);
    final depth = useState(compact ? 1 : 2);
    final hiddenEdges = useState<Set<EdgeClass>>(const {});
    final selectedNode = useState<String?>(null);
    final selectedEdge = useState<int?>(null);
    final transform = useTransformationController();

    final async = ref.watch(localGraphProvider(center.value, depth.value));
    final graph = async.value;

    Widget body;
    GraphNode? focusNode;
    if (graph == null) {
      body = async.hasError
          ? MapsError(error: async.error!)
          : const MapsLoading();
    } else if (!graph.found || graph.nodes.isEmpty) {
      body = StrataEmptyState(
        icon: Icons.hub_outlined,
        title: l10n.mindMapNotFound,
        message: l10n.mindMapNotFoundMessage,
      );
    } else {
      focusNode = graph.nodes.first;
      body = _Canvas(
        graph: graph,
        compact: compact,
        transform: transform,
        hiddenEdges: hiddenEdges.value,
        selectedNode: selectedNode.value,
        selectedEdge: selectedEdge.value,
        onTapNode: (node) {
          selectedEdge.value = null;
          if (compact) {
            center.value = node.id;
          } else {
            selectedNode.value = node.id;
          }
        },
        onTapEdge: (edge, scene) {
          selectedEdge.value = edge;
          if (compact) {
            unawaited(
              showModalBottomSheet<void>(
                context: context,
                builder: (sheetContext) => MapsLocalizationScope(
                  child: EdgeDetails(
                    edge: scene.edges[edge],
                    from: scene.nodes[scene.edgeSrc[edge]].title,
                    to: scene.nodes[scene.edgeDst[edge]].title,
                    onClose: () => Navigator.of(sheetContext).pop(),
                  ),
                ),
              ).whenComplete(() {
                if (context.mounted) selectedEdge.value = null;
              }),
            );
          }
        },
        onCloseEdge: () => selectedEdge.value = null,
      );
    }

    final depthControl = _DepthControl(
      depth: depth.value,
      onChanged: (value) => depth.value = value,
    );

    if (compact) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _CompactBar(
            title: focusNode?.title ?? '',
            onBack: onBack,
            depth: depthControl,
          ),
          if (graph != null && graph.found) _Counts(graph: graph),
          Expanded(child: body),
        ],
      );
    }

    GraphNode? selected;
    if (graph != null) {
      for (final node in graph.nodes) {
        if (node.id == selectedNode.value) selected = node;
      }
    }
    final aside = selected == null
        ? null
        : NodeAside(
            node: selected,
            onOpenNote: onOpenNote,
            onClear: () => selectedNode.value = null,
            onCentre: () {
              center.value = selected!.id;
              selectedNode.value = null;
            },
          );
    final main = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Toolbar(
          title: focusNode?.title ?? '',
          transform: transform,
          onRecentre: focusNode == null
              ? null
              : () => center.value = focusNode!.id,
        ),
        Divider(height: 1, color: colors.border),
        _FilterRow(
          depth: depthControl,
          hidden: hiddenEdges.value,
          onToggle: (cls) => hiddenEdges.value = hiddenEdges.value.contains(cls)
              ? ({...hiddenEdges.value}..remove(cls))
              : {...hiddenEdges.value, cls},
        ),
        Divider(height: 1, color: colors.border),
        if (graph != null && graph.found) _Counts(graph: graph),
        Expanded(child: body),
        const _DragHint(),
      ],
    );
    if (sizeClass == SizeClass.expanded) {
      return ColoredBox(
        color: colors.surface,
        child: Row(
          children: [
            Expanded(child: main),
            if (aside != null) ...[
              VerticalDivider(width: 1, color: colors.border),
              SizedBox(width: StrataLayout.contextPanelWidth, child: aside),
            ],
          ],
        ),
      );
    }
    return ColoredBox(
      color: colors.surface,
      child: Stack(
        children: [
          Positioned.fill(child: main),
          if (aside != null) ...[
            Positioned.fill(
              child: ModalBarrier(
                color: colors.scrim,
                semanticsLabel: l10n.clearSelection,
                onDismiss: () => selectedNode.value = null,
              ),
            ),
            PositionedDirectional(
              top: 0,
              bottom: 0,
              end: 0,
              width: StrataLayout.contextPanelWidth,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: colors.surface,
                  boxShadow: StrataElevation.popover,
                ),
                child: Material(type: MaterialType.transparency, child: aside),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _Canvas extends HookWidget {
  const new({
    required this.graph,
    required this.compact,
    required this.transform,
    required this.hiddenEdges,
    required this.selectedNode,
    required this.selectedEdge,
    required this.onTapNode,
    required this.onTapEdge,
    required this.onCloseEdge,
  });

  final LocalGraphView graph;
  final bool compact;
  final TransformationController transform;
  final Set<EdgeClass> hiddenEdges;
  final String? selectedNode;
  final int? selectedEdge;
  final ValueChanged<GraphNode> onTapNode;
  final void Function(int edge, GraphScene scene) onTapEdge;
  final VoidCallback onCloseEdge;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final scene = useMemoized(() => GraphScene.from(graph.nodes, graph.edges), [
      graph,
    ]);
    final cardSize = compact ? const Size(132, 52) : const Size(168, 56);
    final layout = useMemoized(
      () => MindMapLayout.of(scene, cardSize: cardSize),
      [scene, cardSize],
    );
    final edge = selectedEdge;
    return LayoutBuilder(
      builder: (context, constraints) {
        final viewport = constraints.biggest;
        return HookBuilder(
          builder: (context) {
            useEffect(() {
              // Centre the focused note whenever the graph or window changes.
              final centre = layout.centreOf(0);
              transform.value = Matrix4.identity()
                ..translateByDouble(
                  viewport.width / 2 - centre.dx,
                  viewport.height / 2 - centre.dy,
                  0,
                  1,
                );
              return null;
            }, [layout, viewport]);
            return Stack(
              children: [
                Positioned.fill(
                  child: Semantics(
                    label: l10n.mindMapSemantics(
                      title: graph.nodes.first.title,
                    ),
                    container: true,
                    explicitChildNodes: true,
                    child: InteractiveViewer(
                      transformationController: transform,
                      constrained: false,
                      boundaryMargin: EdgeInsets.all(
                        math.max(viewport.width, viewport.height),
                      ),
                      minScale: 0.3,
                      child: MindMapCanvas(
                        layout: layout,
                        centerId: graph.center,
                        hiddenEdges: hiddenEdges,
                        selectedNodeId: selectedNode,
                        selectedEdge: edge,
                        onTapNode: (i) => onTapNode(scene.nodes[i]),
                        onTapEdge: (e) => onTapEdge(e, scene),
                      ),
                    ),
                  ),
                ),
                if (compact)
                  PositionedDirectional(
                    end: StrataSpacing.s4,
                    bottom: StrataSpacing.s4,
                    child: _FitButton(
                      transform: transform,
                      layout: layout,
                      viewport: viewport,
                    ),
                  ),
                if (!compact && edge != null && edge < scene.edgeCount)
                  PositionedDirectional(
                    top: StrataSpacing.s4,
                    end: StrataSpacing.s4,
                    width: 300,
                    child: Material(
                      color: context.strataColors.surface,
                      shape: RoundedRectangleBorder(
                        borderRadius: StrataRadii.cardRadius,
                        side: BorderSide(color: context.strataColors.border),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.only(top: StrataSpacing.s2),
                        child: CallbackShortcuts(
                          bindings: {
                            const SingleActivator(LogicalKeyboardKey.escape):
                                onCloseEdge,
                          },
                          child: EdgeDetails(
                            edge: scene.edges[edge],
                            from: scene.nodes[scene.edgeSrc[edge]].title,
                            to: scene.nodes[scene.edgeDst[edge]].title,
                            onClose: onCloseEdge,
                          ),
                        ),
                      ),
                    ),
                  ),
              ],
            );
          },
        );
      },
    );
  }
}

class _FitButton extends StatelessWidget {
  const new({
    required this.transform,
    required this.layout,
    required this.viewport,
  });

  final TransformationController transform;
  final MindMapLayout layout;
  final Size viewport;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return IconButton.outlined(
      tooltip: context.mapsL10n.zoomToFit,
      style: IconButton.styleFrom(
        backgroundColor: colors.surface,
        minimumSize: const Size.square(48),
      ),
      onPressed: () => fitMindMap(transform, layout.size, viewport),
      icon: const Icon(Icons.fit_screen_outlined),
    );
  }
}

/// Fits a mind map canvas of [content] size in [viewport].
void fitMindMap(
  TransformationController transform,
  Size content,
  Size viewport,
) {
  final scale = math
      .min(viewport.width / content.width, viewport.height / content.height)
      .clamp(0.3, 1.0);
  transform.value = Matrix4.identity()
    ..translateByDouble(
      (viewport.width - content.width * scale) / 2,
      (viewport.height - content.height * scale) / 2,
      0,
      1,
    )
    ..scaleByDouble(scale, scale, 1, 1);
}

/// Zooms a mind map by [factor] around the viewport centre.
void zoomMindMap(
  TransformationController transform,
  double factor,
  Size viewport,
) {
  final centre = Offset(viewport.width / 2, viewport.height / 2);
  final scene = transform.toScene(centre);
  final current = transform.value.getMaxScaleOnAxis();
  final next = (current * factor).clamp(0.3, 2.5);
  transform.value = Matrix4.identity()
    ..translateByDouble(
      centre.dx - scene.dx * next,
      centre.dy - scene.dy * next,
      0,
      1,
    )
    ..scaleByDouble(next, next, 1, 1);
}

class _DepthControl extends StatelessWidget {
  const new({required this.depth, required this.onChanged});

  final int depth;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          l10n.depth,
          style: context.strataText.caption.copyWith(
            color: context.strataColors.text2,
          ),
        ),
        const SizedBox(width: StrataSpacing.s2),
        SegmentedButton<int>(
          showSelectedIcon: false,
          style: const ButtonStyle(visualDensity: VisualDensity.compact),
          segments: [
            for (final value in const [1, 2, 3])
              ButtonSegment(
                value: value,
                label: Semantics(
                  label: l10n.depthValue(depth: value),
                  excludeSemantics: true,
                  child: Text('$value'),
                ),
              ),
          ],
          selected: {depth},
          onSelectionChanged: (values) => onChanged(values.first),
        ),
      ],
    );
  }
}

class _CompactBar extends StatelessWidget {
  const new({required this.title, required this.onBack, required this.depth});

  final String title;
  final VoidCallback? onBack;
  final Widget depth;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final text = context.strataText;
    final back = onBack;
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s2,
        vertical: StrataSpacing.s1,
      ),
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        alignment: WrapAlignment.spaceBetween,
        runSpacing: StrataSpacing.s1,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (back != null)
                IconButton(
                  tooltip: l10n.backToNote,
                  onPressed: back,
                  icon: const BackButtonIcon(),
                ),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 220),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Semantics(
                      header: true,
                      child: Text(
                        title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: text.titleSmall,
                      ),
                    ),
                    Text(
                      l10n.localMap,
                      style: text.caption.copyWith(
                        color: context.strataColors.text2,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          depth,
        ],
      ),
    );
  }
}

class _Toolbar extends StatelessWidget {
  const new({
    required this.title,
    required this.transform,
    required this.onRecentre,
  });

  final String title;
  final TransformationController transform;
  final VoidCallback? onRecentre;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final text = context.strataText;
    final colors = context.strataColors;
    final recentre = onRecentre;
    return LayoutBuilder(
      builder: (context, constraints) {
        final viewport = Size(constraints.maxWidth, 600);
        return Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: StrataSpacing.s5,
            vertical: StrataSpacing.s2,
          ),
          child: Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: StrataSpacing.s3,
            runSpacing: StrataSpacing.s2,
            children: [
              Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.mapBreadcrumb,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                  Semantics(
                    header: true,
                    child: Text(title, style: text.titleSmall),
                  ),
                ],
              ),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  IconButton(
                    tooltip: l10n.zoomOut,
                    onPressed: () => zoomMindMap(transform, 0.8, viewport),
                    icon: const Icon(Icons.remove),
                  ),
                  IconButton(
                    tooltip: l10n.zoomIn,
                    onPressed: () => zoomMindMap(transform, 1.25, viewport),
                    icon: const Icon(Icons.add),
                  ),
                  IconButton(
                    tooltip: l10n.recentreOn(title: title),
                    onPressed: recentre,
                    icon: const Icon(Icons.center_focus_strong_outlined),
                  ),
                  const SizedBox(width: StrataSpacing.s2),
                  Tooltip(
                    message: l10n.saveLayoutUnavailable,
                    child: OutlinedButton(
                      onPressed: null,
                      style: OutlinedButton.styleFrom(
                        disabledForegroundColor: colors.text2,
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(l10n.saveLayout),
                          const SizedBox(width: StrataSpacing.s1),
                          Text(l10n.saveLayoutTarget, style: text.monoSmall),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        );
      },
    );
  }
}

class _FilterRow extends StatelessWidget {
  const new({
    required this.depth,
    required this.hidden,
    required this.onToggle,
  });

  final Widget depth;
  final Set<EdgeClass> hidden;
  final ValueChanged<EdgeClass> onToggle;

  @override
  Widget build(BuildContext context) {
    final shared = context.l10n;
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s5,
        vertical: StrataSpacing.s2,
      ),
      child: Wrap(
        spacing: StrataSpacing.s2,
        runSpacing: StrataSpacing.s1,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          depth,
          const SizedBox(width: StrataSpacing.s2),
          for (final cls in filterableEdgeClasses)
            if (cls != EdgeClass.bodyLink)
              FilterChip(
                showCheckmark: false,
                avatar: RelationLineSample(type: relationTypeOf(cls)),
                label: Text(shared.relationTypeLabel(relationTypeOf(cls))),
                selected: !hidden.contains(cls),
                onSelected: (_) => onToggle(cls),
              ),
        ],
      ),
    );
  }
}

class _Counts extends StatelessWidget {
  const new({required this.graph});

  final LocalGraphView graph;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsetsDirectional.fromSTEB(
      StrataSpacing.s4,
      StrataSpacing.s2,
      StrataSpacing.s4,
      0,
    ),
    child: Align(
      alignment: AlignmentDirectional.centerStart,
      child: StatusPill(
        icon: Icons.hub_outlined,
        label: context.mapsL10n.mindMapCounts(
          depth: graph.depth,
          nodes: graph.nodes.length,
          edges: graph.edges.length,
        ),
      ),
    ),
  );
}

class _DragHint extends StatelessWidget {
  const new();

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.all(StrataSpacing.s3),
      child: Center(
        child: Semantics(
          container: true,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border.all(color: colors.border),
              borderRadius: StrataRadii.pillRadius,
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: StrataSpacing.s4,
                vertical: StrataSpacing.s2,
              ),
              child: Text.rich(
                TextSpan(
                  children: [
                    TextSpan(
                      text: l10n.dragHint,
                      style: text.bodySmall.withWeight(FontWeight.w600),
                    ),
                    const TextSpan(text: ' '),
                    TextSpan(text: l10n.dragHintMore),
                  ],
                ),
                textAlign: TextAlign.center,
                style: text.caption.copyWith(color: colors.text2),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
