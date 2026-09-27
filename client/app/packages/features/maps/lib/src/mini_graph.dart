import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/graph_camera.dart';
import 'package:strata_maps/src/graph/graph_painter.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_maps/src/graph/graph_viewport.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// A small, static painting of a note's or entity's neighbourhood (depth 1,
/// the core's local layout) for context panels, with an "Open in map" action.
class MiniGraph extends StatelessWidget {
  /// Creates the mini graph of [noteId].
  const new(
    this.noteId, {
    super.key,
    this.height = 200,
    this.onOpenMindMap,
    this.showHeader = true,
  });

  /// The note or entity in the centre.
  final String noteId;

  /// Canvas height.
  final double height;

  /// Opens the local mind map (its ID).
  final ValueChanged<String>? onOpenMindMap;

  /// Whether the "Graph · Open in map" header is shown.
  final bool showHeader;

  @override
  Widget build(BuildContext context) => MapsLocalizationScope(
    child: _MiniGraph(
      noteId: noteId,
      height: height,
      onOpenMindMap: onOpenMindMap,
      showHeader: showHeader,
    ),
  );
}

class _MiniGraph extends HookConsumerWidget {
  const new({
    required this.noteId,
    required this.height,
    required this.onOpenMindMap,
    required this.showHeader,
  });

  final String noteId;
  final double height;
  final ValueChanged<String>? onOpenMindMap;
  final bool showHeader;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final graph = ref.watch(localGraphProvider(noteId, 1)).value;
    final view = useMemoized(
      () => GraphViewController()
        ..fitPadding = 28
        ..fitMaxScale = 1.2,
    );
    useEffect(() => view.dispose, [view]);
    final scene = useMemoized(
      () => graph == null ? null : GraphScene.from(graph.nodes, graph.edges),
      [graph],
    );
    final open = onOpenMindMap;
    final Widget canvas;
    if (scene == null || scene.nodeCount == 0) {
      canvas = SizedBox(height: height);
    } else {
      canvas = SizedBox(
        height: height,
        child: GraphViewport(
          scene: scene,
          view: view,
          interactive: false,
          clusterRegions: false,
          options: GraphPaintOptions(selected: scene.index[noteId]),
          semanticLabel: l10n.miniGraphSemantics(
            title: scene.nodes.first.title,
            count: scene.nodeCount - 1,
          ),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (showHeader)
          Padding(
            padding: const EdgeInsetsDirectional.only(
              start: StrataSpacing.s4,
              end: StrataSpacing.s2,
            ),
            child: Row(
              children: [
                Expanded(
                  child: Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: Semantics(
                      header: true,
                      child: Text(
                        l10n.entityGraph,
                        style: context.strataText.caption
                            .withWeight(FontWeight.w700)
                            .copyWith(color: colors.text2, letterSpacing: 0.4),
                      ),
                    ),
                  ),
                ),
                if (open != null)
                  Flexible(
                    child: TextButton(
                      onPressed: () => open(noteId),
                      child: Text(
                        l10n.openInMap,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s4),
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border.all(color: colors.border),
              borderRadius: StrataRadii.cardRadius,
            ),
            child: ClipRRect(
              borderRadius: StrataRadii.cardRadius,
              child: canvas,
            ),
          ),
        ),
      ],
    );
  }
}
