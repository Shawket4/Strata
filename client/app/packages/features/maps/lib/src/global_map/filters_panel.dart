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
  });

  /// Edge classes switched off.
  final Set<EdgeClass> hiddenEdges;

  /// Node kinds switched off.
  final Set<NodeKind> hiddenKinds;

  /// Cluster in focus.
  final String? focusCluster;

  /// A copy with [edge] toggled.
  MapFilters toggleEdge(EdgeClass edge) => MapFilters(
    hiddenEdges: hiddenEdges.contains(edge)
        ? ({...hiddenEdges}..remove(edge))
        : {...hiddenEdges, edge},
    hiddenKinds: hiddenKinds,
    focusCluster: focusCluster,
  );

  /// A copy with [kind] toggled.
  MapFilters toggleKind(NodeKind kind) => MapFilters(
    hiddenEdges: hiddenEdges,
    hiddenKinds: hiddenKinds.contains(kind)
        ? ({...hiddenKinds}..remove(kind))
        : {...hiddenKinds, kind},
    focusCluster: focusCluster,
  );

  /// A copy focused on [cluster] (`null`: all clusters).
  MapFilters focus(String? cluster) => MapFilters(
    hiddenEdges: hiddenEdges,
    hiddenKinds: hiddenKinds,
    focusCluster: cluster,
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
    this.onClose,
  });

  /// The core's clusters (by name).
  final List<ClusterLabel> clusters;

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
      child: Semantics(header: true, child: Text(label, style: legend)),
    );
    String edgeLabel(EdgeClass edge) => edge == EdgeClass.bodyLink
        ? l10n.edgeBodyLinks
        : shared.relationTypeLabel(relationTypeOf(edge));
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
                        Flexible(child: Text(edgeLabel(edge))),
                      ],
                    ),
                  ),
                const Divider(height: StrataSpacing.s4),
                Tooltip(
                  message: l10n.notAvailableYet,
                  child: SwitchListTile(
                    value: false,
                    onChanged: null,
                    title: Row(
                      children: [
                        const RelationLineSample(
                          type: RelationType.similarity,
                          width: 28,
                        ),
                        const SizedBox(width: StrataSpacing.s2),
                        Flexible(
                          child: Text(
                            l10n.similarityTitle,
                            style: TextStyle(color: colors.text2),
                          ),
                        ),
                      ],
                    ),
                    subtitle: Text(
                      l10n.similarityHelp,
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ),
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
                          label: Text(shared.nodeKindLabel(kind)),
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
