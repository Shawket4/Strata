import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// Ephemeral map filter choices.
@immutable
class MapFilters {
  /// Creates filter choices.
  const new({
    this.hiddenEdges = const {},
    this.hiddenKinds = const {},
    this.focusCluster,
    this.similarity = false,
    this.includeTags = false,
    this.lens = GraphLens.notes,
  });

  /// Edge classes switched off.
  final Set<EdgeClass> hiddenEdges;

  /// Node kinds switched off.
  final Set<NodeKind> hiddenKinds;

  /// Cluster in focus.
  final String? focusCluster;

  /// Whether the AI similarity edges are shown.
  final bool similarity;

  /// Whether tags are shown as nodes.
  final bool includeTags;

  /// Notes, people or companies.
  final GraphLens lens;

  MapFilters _copy({
    Set<EdgeClass>? hiddenEdges,
    Set<NodeKind>? hiddenKinds,
    String? Function()? focusCluster,
    bool? similarity,
    bool? includeTags,
    GraphLens? lens,
  }) => MapFilters(
    hiddenEdges: hiddenEdges ?? this.hiddenEdges,
    hiddenKinds: hiddenKinds ?? this.hiddenKinds,
    focusCluster: focusCluster == null ? this.focusCluster : focusCluster(),
    similarity: similarity ?? this.similarity,
    includeTags: includeTags ?? this.includeTags,
    lens: lens ?? this.lens,
  );

  /// A copy with [edge] toggled.
  MapFilters toggleEdge(EdgeClass edge) => _copy(
    hiddenEdges: hiddenEdges.contains(edge)
        ? ({...hiddenEdges}..remove(edge))
        : {...hiddenEdges, edge},
  );

  /// A copy with [kind] toggled.
  MapFilters toggleKind(NodeKind kind) => _copy(
    hiddenKinds: hiddenKinds.contains(kind)
        ? ({...hiddenKinds}..remove(kind))
        : {...hiddenKinds, kind},
  );

  /// A copy with the similarity edges on or off.
  MapFilters withSimilarity({required bool on}) => _copy(similarity: on);

  /// A copy with tag nodes on or off.
  MapFilters withTags({required bool on}) => _copy(includeTags: on);

  /// A copy through [value]'s lens.
  MapFilters withLens(GraphLens value) => _copy(lens: value);

  /// The core's map query for these choices: the core drops the hidden edge
  /// classes and node kinds (L15) and lists the neighbours of [focus] (the
  /// selected node) for highlighting; the cluster focus only dims
  /// (painting).
  GraphFilter toCore({String? focus}) => GraphFilter(
    edgeKinds: selectedEdgeKinds(hiddenEdges),
    nodeKinds: hiddenKinds.isEmpty
        ? const []
        : [
            for (final kind in filterableNodeKinds)
              if (!hiddenKinds.contains(kind)) kind.name,
          ],
    similarity: similarity,
    lens: lens,
    focus: focus,
    includeTags: includeTags,
  );

  /// A copy focused on [cluster] (`null`: all clusters).
  MapFilters focus(String? cluster) => _copy(focusCluster: () => cluster);

  @override
  bool operator ==(Object other) =>
      other is MapFilters &&
      setEquals(other.hiddenEdges, hiddenEdges) &&
      setEquals(other.hiddenKinds, hiddenKinds) &&
      other.focusCluster == focusCluster &&
      other.similarity == similarity &&
      other.includeTags == includeTags &&
      other.lens == lens;

  @override
  int get hashCode => Object.hash(
    Object.hashAllUnordered(hiddenEdges),
    Object.hashAllUnordered(hiddenKinds),
    focusCluster,
    similarity,
    includeTags,
    lens,
  );
}

/// The map filters: edge types, AI similarity, node kinds and cluster focus
/// (expanded: a side panel; medium: an overlay drawer).
class MapFiltersPanel extends StatelessWidget {
  /// Creates the panel.
  const new({
    required this.clusters,
    required this.filters,
    required this.onChanged,
    super.key,
    this.edgeCounts = const [],
    this.nodeCounts = const [],
    this.similarity = Availability.available,
    this.onShowSimilarity,
    this.onClose,
  });

  /// The core's clusters (by name).
  final List<ClusterLabel> clusters;

  /// Edges per kind before filtering (the core's counts).
  final List<KindCount> edgeCounts;

  /// Nodes per kind before filtering.
  final List<KindCount> nodeCounts;

  /// Whether the server's similarity edges can be shown.
  final Availability similarity;

  /// Fetches the similarity edges before they are switched on.
  final VoidCallback? onShowSimilarity;

  /// Current choices.
  final MapFilters filters;

  /// New choices.
  final ValueChanged<MapFilters> onChanged;

  /// Closes the drawer (medium only).
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final shared = context.l10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final close = onClose;
    final legend = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2, letterSpacing: 0.4);
    Widget heading(String label) => Padding(
      padding: const EdgeInsetsDirectional.fromSTEB(
        StrataSpacing.s4,
        StrataSpacing.s4,
        StrataSpacing.s4,
        StrataSpacing.s1,
      ),
      child: Semantics(
        header: true,
        container: true,
        child: Text(label, style: legend),
      ),
    );
    String edgeLabel(EdgeClass edge) => edge == EdgeClass.bodyLink
        ? l10n.edgeBodyLinks
        : shared.relationTypeLabel(relationTypeOf(edge));
    // The core's count of a kind (looked up, never computed here).
    String? countOf(List<KindCount> counts, String kind) {
      for (final count in counts) {
        if (count.kind == kind) return '${count.count}';
      }
      return null;
    }

    return Material(
      type: MaterialType.transparency,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsetsDirectional.fromSTEB(
              StrataSpacing.s4,
              StrataSpacing.s3,
              StrataSpacing.s1,
              0,
            ),
            child: Row(
              children: [
                Expanded(
                  child: Semantics(
                    header: true,
                    container: true,
                    child: Text(
                      close == null ? l10n.filters : l10n.mapFilters,
                      style: text.titleSmall,
                    ),
                  ),
                ),
                TextButton(
                  onPressed: () => onChanged(const MapFilters()),
                  child: Text(l10n.resetFilters),
                ),
                if (close != null)
                  IconButton(
                    tooltip: l10n.closeFilters,
                    onPressed: close,
                    icon: const Icon(Icons.close),
                  ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.only(bottom: StrataSpacing.s4),
              children: [
                heading(l10n.edgeTypes),
                for (final edge in filterableEdgeClasses)
                  CheckboxListTile(
                    dense: true,
                    controlAffinity: ListTileControlAffinity.leading,
                    value: !filters.hiddenEdges.contains(edge),
                    onChanged: (_) => onChanged(filters.toggleEdge(edge)),
                    title: Row(
                      children: [
                        RelationLineSample(
                          type: relationTypeOf(edge),
                          width: 28,
                        ),
                        const SizedBox(width: StrataSpacing.s2),
                        Expanded(child: Text(edgeLabel(edge))),
                        if (countOf(edgeCounts, coreEdgeKindsOf(edge).first)
                            case final count?)
                          Text(
                            count,
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                      ],
                    ),
                  ),
                const Divider(height: StrataSpacing.s4),
                SwitchListTile(
                  value: filters.similarity,
                  onChanged: similarity == Availability.available
                      ? (on) {
                          if (on) onShowSimilarity?.call();
                          onChanged(filters.withSimilarity(on: on));
                        }
                      : null,
                  title: Row(
                    children: [
                      const RelationLineSample(
                        type: RelationType.similarity,
                        width: 28,
                      ),
                      const SizedBox(width: StrataSpacing.s2),
                      Flexible(child: Text(l10n.similarityTitle)),
                    ],
                  ),
                  subtitle: Text(
                    similarity == Availability.offline
                        ? l10n.similarityOffline
                        : l10n.similarityHelp,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                ),
                SwitchListTile(
                  value: filters.includeTags,
                  onChanged: (on) => onChanged(filters.withTags(on: on)),
                  title: Text(l10n.showTags),
                ),
                const Divider(height: StrataSpacing.s4),
                heading(l10n.nodeKinds),
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s4,
                  ),
                  child: Wrap(
                    spacing: StrataSpacing.s2,
                    runSpacing: StrataSpacing.s1,
                    children: [
                      for (final kind in filterableNodeKinds)
                        FilterChip(
                          avatar: NodeKindGlyph(kind: kind, decorative: true),
                          label: Text(switch (countOf(nodeCounts, kind.name)) {
                            final count? => l10n.kindWithCount(
                              kind: shared.nodeKindLabel(kind),
                              count: count,
                            ),
                            null => shared.nodeKindLabel(kind),
                          }),
                          selected: !filters.hiddenKinds.contains(kind),
                          showCheckmark: false,
                          onSelected: (_) =>
                              onChanged(filters.toggleKind(kind)),
                        ),
                    ],
                  ),
                ),
                const Divider(height: StrataSpacing.s6),
                heading(l10n.clusterFocus),
                RadioGroup<String?>(
                  groupValue: filters.focusCluster,
                  onChanged: (value) => onChanged(filters.focus(value)),
                  child: Column(
                    children: [
                      RadioListTile<String?>(
                        dense: true,
                        value: null,
                        title: Text(l10n.allClusters),
                      ),
                      for (final cluster in clusters)
                        RadioListTile<String?>(
                          dense: true,
                          value: cluster.id,
                          title: Text(cluster.name),
                          secondary: Text(
                            l10n.clusterMembers(count: cluster.size),
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
