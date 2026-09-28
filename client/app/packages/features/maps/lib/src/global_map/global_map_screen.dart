import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_maps/src/common/async_states.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/global_map/filters_panel.dart';
import 'package:strata_maps/src/global_map/map_overlays.dart';
import 'package:strata_maps/src/graph/graph_camera.dart';
import 'package:strata_maps/src/graph/graph_painter.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_maps/src/graph/graph_viewport.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The global map (PLAN §11 screen 5): the whole vault on one
/// [CustomPainter] with the core's positions (D3 = a), clusters as regions,
/// zoom-dependent labels, filters, search-to-focus and selection with
/// dimming. Medium: filters as an overlay drawer; expanded: filters panel and
/// minimap. Compact shows local mind maps only, so it renders a notice.
class GlobalMapScreen extends StatelessWidget {
  /// Creates the global map.
  const new({super.key, this.onOpenNote, this.onOpenMindMap});

  /// The icon that represents this feature.
  static const IconData icon = Icons.hub_outlined;

  /// Opens a note (its ID).
  final ValueChanged<String>? onOpenNote;

  /// Opens a node's local mind map (its ID).
  final ValueChanged<String>? onOpenMindMap;

  @override
  Widget build(BuildContext context) => MapsLocalizationScope(
    child: _GlobalMap(onOpenNote: onOpenNote, onOpenMindMap: onOpenMindMap),
  );
}

class _GlobalMap extends HookConsumerWidget {
  const new({required this.onOpenNote, required this.onOpenMindMap});

  final ValueChanged<String>? onOpenNote;
  final ValueChanged<String>? onOpenMindMap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final filters = useState(const MapFilters());
    // One query object per filter choice, so the provider keeps its key.
    final query = useMemoized(() => filters.value.toCore(), [filters.value]);
    final shown = useRef<GlobalGraphView?>(null);
    if (SizeClass.of(context) == SizeClass.compact) {
      return StrataEmptyState(
        icon: GlobalMapScreen.icon,
        title: l10n.mapCompactTitle,
        message: l10n.mapCompactMessage,
      );
    }
    final graph = ref.watch(globalGraphFilteredProvider(query));
    if (graph case AsyncData(:final value)) shown.value = value;
    // While a new filter choice loads, the previous map stays up.
    final view = shown.value;
    return ColoredBox(
      color: context.strataColors.surface,
      child: switch (graph) {
        AsyncError(:final error) => MapsError(error: error),
        _ when view == null => const MapsLoading(),
        _ when view.nodeCounts.isEmpty => StrataEmptyState(
          icon: GlobalMapScreen.icon,
          title: l10n.mapEmptyTitle,
          message: l10n.mapEmptyMessage,
        ),
        _ => _MapBody(
          view: view,
          filters: filters.value,
          onFiltersChanged: (value) => filters.value = value,
          onOpenNote: onOpenNote,
          onOpenMindMap: onOpenMindMap,
        ),
      },
    );
  }
}

class _MapBody extends HookConsumerWidget {
  const new({
    required this.view,
    required this.filters,
    required this.onFiltersChanged,
    required this.onOpenNote,
    required this.onOpenMindMap,
  });

  final GlobalGraphView view;
  final MapFilters filters;
  final ValueChanged<MapFilters> onFiltersChanged;
  final ValueChanged<String>? onOpenNote;
  final ValueChanged<String>? onOpenMindMap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final scene = useMemoized(() => GraphScene.from(view.nodes, view.edges), [
      view,
    ]);
    final camera = useMemoized(GraphViewController.new);
    useEffect(() => camera.dispose, [camera]);
    // By node ID: a new filter choice rebuilds the scene (new indices).
    final selected = useState<String?>(null);
    final hovered = useState<(int, Offset)?>(null);
    useValueChanged<GlobalGraphView, void>(
      view,
      (_, _) => hovered.value = null,
    );
    final filtersOpen = useState(false);
    final searchFocus = useFocusNode();
    final expanded = SizeClass.of(context) == SizeClass.expanded;

    final selectedId = selected.value;
    final selectedIndex = selectedId == null ? null : scene.index[selectedId];
    final selectedNode = selectedIndex == null
        ? null
        : scene.nodes[selectedIndex];
    Set<int>? highlight;
    if (selectedNode != null) {
      final neighbourhood = ref.watch(localGraphProvider(selectedNode.id, 1));
      final ids = neighbourhood.value?.nodes ?? const <GraphNode>[];
      highlight = {
        selectedIndex!,
        for (final node in ids) ?scene.index[node.id],
      };
    }
    final options = GraphPaintOptions(
      focusCluster: filters.focusCluster,
      selected: selectedIndex,
      highlight: highlight,
      hovered: hovered.value?.$1,
    );

    void select(int? index) {
      selected.value = index == null ? null : scene.nodes[index].id;
    }

    void focusOn(String id) {
      final index = scene.index[id];
      if (index == null) return;
      selected.value = id;
      camera.centreOn(scene.positionOf(index), minZoom: 1);
    }

    final canvas = GraphViewport(
      scene: scene,
      view: camera,
      semanticLabel: l10n.mapSemantics(
        nodes: scene.nodeCount,
        edges: scene.edgeCount,
      ),
      options: options,
      onTapNode: select,
      onHoverNode: (node, position) =>
          hovered.value = node == null ? null : (node, position),
      onEscape: () => select(null),
      shortcuts: {
        const SingleActivator(LogicalKeyboardKey.slash):
            searchFocus.requestFocus,
      },
    );

    final hover = hovered.value;
    final panel = MapFiltersPanel(
      clusters: view.clusters,
      filters: filters,
      onChanged: onFiltersChanged,
      onClose: expanded ? null : () => filtersOpen.value = false,
    );

    final search = MapSearchField(onFocusNode: focusOn, focusNode: searchFocus);
    final stack = Stack(
      children: [
        Positioned.fill(child: canvas),
        if (hover != null)
          HoverCard(node: scene.nodes[hover.$1], position: hover.$2),
        if (!expanded)
          PositionedDirectional(
            top: StrataSpacing.s4,
            start: StrataSpacing.s4,
            end: StrataSpacing.s4,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Flexible(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 320),
                    child: search,
                  ),
                ),
                const SizedBox(width: StrataSpacing.s2),
                FilledButton.tonalIcon(
                  onPressed: () => filtersOpen.value = !filtersOpen.value,
                  icon: const Icon(Icons.tune, size: 18),
                  label: Text(l10n.filters),
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(48, 48),
                    backgroundColor: colors.accentTint,
                    foregroundColor: colors.accentText,
                  ),
                ),
              ],
            ),
          ),
        if (selectedNode != null)
          PositionedDirectional(
            top: expanded ? StrataSpacing.s4 : 80,
            start: StrataSpacing.s4,
            end: StrataSpacing.s4,
            child: Align(
              alignment: AlignmentDirectional.topStart,
              child: FocusBar(
                title: selectedNode.title,
                onClear: () => select(null),
                onOpenNote: onOpenNote == null
                    ? null
                    : () => onOpenNote!(selectedNode.id),
                onOpenMindMap: onOpenMindMap == null
                    ? null
                    : () => onOpenMindMap!(selectedNode.id),
              ),
            ),
          ),
        PositionedDirectional(
          start: StrataSpacing.s4,
          bottom: StrataSpacing.s4,
          child: ZoomControls(view: camera),
        ),
        if (expanded)
          PositionedDirectional(
            end: StrataSpacing.s4,
            bottom: StrataSpacing.s4,
            child: Minimap(scene: scene, view: camera),
          ),
        if (!expanded && filtersOpen.value) ...[
          Positioned.fill(
            child: ModalBarrier(
              color: colors.scrim,
              semanticsLabel: l10n.closeFilters,
              onDismiss: () => filtersOpen.value = false,
            ),
          ),
          PositionedDirectional(
            top: 0,
            bottom: 0,
            end: 0,
            width: 320,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: colors.surface,
                boxShadow: StrataElevation.popover,
              ),
              child: panel,
            ),
          ),
        ],
      ],
    );

    if (!expanded) return stack;
    return Row(
      children: [
        Expanded(
          child: Column(
            children: [
              MapHeader(
                counts: l10n.mapCounts(
                  nodes: scene.nodeCount,
                  edges: scene.edgeCount,
                  clusters: view.clusters.length,
                ),
                search: search,
              ),
              Divider(height: 1, color: colors.border),
              Expanded(child: stack),
            ],
          ),
        ),
        VerticalDivider(width: 1, color: colors.border),
        SizedBox(width: StrataLayout.contextPanelWidth, child: panel),
      ],
    );
  }
}
