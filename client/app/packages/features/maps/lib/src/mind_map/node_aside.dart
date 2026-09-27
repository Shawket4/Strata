import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The vault's relation type string → design-system relation type (1:1).
RelationType relationTypeOfWire(String relType) => switch (relType) {
  'part-of' => RelationType.partOf,
  'supports' => RelationType.supports,
  'contradicts' => RelationType.contradicts,
  'follows-up' => RelationType.followsUp,
  'duplicates' => RelationType.duplicates,
  _ => RelationType.related,
};

/// The selected node's panel on the mind map (expanded aside, medium
/// drawer): path, title, the note's relations and backlinks (from the
/// core's note view), and open / centre actions.
class NodeAside extends ConsumerWidget {
  /// Creates the panel for [node].
  const new({
    required this.node,
    required this.onClear,
    required this.onCentre,
    super.key,
    this.onOpenNote,
  });

  /// The selected node.
  final GraphNode node;

  /// Clears the selection.
  final VoidCallback onClear;

  /// Recentres the map on the node.
  final VoidCallback onCentre;

  /// Opens the note.
  final ValueChanged<String>? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final note = ref.watch(noteProvider(node.id)).value?.note;
    final open = onOpenNote;
    final heading = text.bodySmall.withWeight(FontWeight.w700);
    return Semantics(
      container: true,
      label: l10n.selectedNode,
      explicitChildNodes: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        note?.path ?? '',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        textDirection: TextDirection.ltr,
                        style: text.monoSmall.copyWith(color: colors.text2),
                      ),
                    ),
                    IconButton(
                      tooltip: l10n.clearSelection,
                      onPressed: onClear,
                      icon: const Icon(Icons.close),
                    ),
                  ],
                ),
                Semantics(
                  header: true,
                  container: true,
                  child: Text(node.title, style: text.title),
                ),
                if (note != null) ...[
                  const SizedBox(height: StrataSpacing.s4),
                  StrataSectionHeader(
                    title: l10n.relations,
                    count: note.relations.length,
                    padding: EdgeInsets.zero,
                  ),
                  const SizedBox(height: StrataSpacing.s2),
                  Wrap(
                    spacing: StrataSpacing.s2,
                    runSpacing: StrataSpacing.s2,
                    children: [
                      for (final relation in note.relations)
                        RelationChip(
                          type: relationTypeOfWire(relation.relType),
                          label: relation.target.title,
                          aiConfidence: relation.by == 'ai'
                              ? relation.confidence
                              : null,
                          onPressed: open == null || relation.target.id == null
                              ? null
                              : () => open(relation.target.id!),
                        ),
                    ],
                  ),
                  const SizedBox(height: StrataSpacing.s4),
                  StrataSectionHeader(
                    title: l10n.backlinks,
                    padding: EdgeInsets.zero,
                  ),
                  for (final group in note.backlinks)
                    for (final item in group.items)
                      Align(
                        alignment: AlignmentDirectional.centerStart,
                        child: TextButton(
                          onPressed: open == null
                              ? null
                              : () => open(item.noteId),
                          child: Text(item.title, style: heading),
                        ),
                      ),
                ],
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (open != null)
                  FilledButton(
                    onPressed: () => open(node.id),
                    child: Text(l10n.openNote),
                  ),
                const SizedBox(height: StrataSpacing.s2),
                OutlinedButton(
                  onPressed: onCentre,
                  child: Text(l10n.centreMapOn(title: node.title)),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
