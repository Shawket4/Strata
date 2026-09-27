import 'package:flutter/material.dart';
import 'package:strata_notes/src/detail/relation_actions.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_state/strata_state.dart' show NoteView;
import 'package:strata_ui/strata_ui.dart';

/// The note's Properties panel: typed relation chips (AI ones with
/// `AI · confidence` and Reject / Retype), tags, and the other frontmatter
/// properties as the core lists them. On compact it starts collapsed to the
/// chips row with a relation count.
class PropertiesPanel extends StatefulWidget {
  /// Creates the panel for [note].
  const new({
    required this.note,
    super.key,
    this.onOpenNote,
    this.collapsible = false,
    this.hoverCards = false,
  });

  /// The note.
  final NoteView note;

  /// Opens a related note.
  final ValueChanged<String>? onOpenNote;

  /// Whether the panel can collapse to its chips (compact).
  final bool collapsible;

  /// Whether AI chips show their reason card on hover (pointer layouts).
  final bool hoverCards;

  @override
  State<PropertiesPanel> createState() => _PropertiesPanelState();
}

class _PropertiesPanelState extends State<PropertiesPanel> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final note = widget.note;
    final expanded = !widget.collapsible || _expanded;
    final chips = Wrap(
      spacing: StrataSpacing.s1 + 2,
      runSpacing: StrataSpacing.s1 + 2,
      children: [
        for (final relation in note.relations)
          NoteRelationChip(
            noteId: note.id,
            relation: relation,
            onOpenNote: widget.onOpenNote,
            hoverCard: widget.hoverCards,
          ),
      ],
    );
    Widget row(String label, Widget value) => Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s2),
      child: Wrap(
        spacing: StrataSpacing.s3,
        runSpacing: StrataSpacing.s1,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          ConstrainedBox(
            constraints: const BoxConstraints(minWidth: 88),
            child: Text(
              label,
              style: text.caption.copyWith(color: colors.text2),
            ),
          ),
          value,
        ],
      ),
    );
    return Semantics(
      container: true,
      label: l10n.propertiesTitle,
      explicitChildNodes: true,
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.fromLTRB(
          StrataSpacing.s3 + 2,
          StrataSpacing.s2,
          StrataSpacing.s3 + 2,
          StrataSpacing.s3,
        ),
        decoration: BoxDecoration(
          color: colors.background,
          border: Border.all(color: colors.border),
          borderRadius: StrataRadii.cardRadius,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Semantics(
                  header: true,
                  child: Text(
                    l10n.propertiesTitle,
                    style: text.caption
                        .copyWith(color: colors.text2)
                        .copyWith(fontWeight: FontWeight.w600),
                  ),
                ),
                const SizedBox(width: StrataSpacing.s2),
                Expanded(
                  child: Text(
                    l10n.propertiesSummary(relations: note.relations.length),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                ),
                if (widget.collapsible)
                  IconButton(
                    tooltip: _expanded
                        ? l10n.collapseProperties
                        : l10n.expandProperties,
                    icon: Icon(
                      _expanded ? Icons.expand_less : Icons.expand_more,
                    ),
                    color: colors.text2,
                    onPressed: () => setState(() => _expanded = !_expanded),
                  ),
              ],
            ),
            if (!expanded && note.relations.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s1),
              chips,
            ],
            if (expanded) ...[
              if (note.relations.isNotEmpty) row(l10n.relationsLabel, chips),
              if (note.tags.isNotEmpty)
                row(
                  l10n.tagsLabel,
                  Wrap(
                    spacing: StrataSpacing.s1 + 2,
                    runSpacing: StrataSpacing.s1,
                    children: [
                      for (final tag in note.tags)
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: StrataSpacing.s2 - 2,
                          ),
                          decoration: BoxDecoration(
                            color: colors.infoTint,
                            borderRadius: StrataRadii.inputRadius,
                          ),
                          child: Text(
                            l10n.tagChip(tag: tag),
                            style: text.bodySmall.copyWith(
                              color: colors.infoText,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              for (final property in note.properties)
                row(
                  property.key,
                  Wrap(
                    spacing: StrataSpacing.s2,
                    children: [
                      for (final value in property.values)
                        Text(
                          value,
                          style: text.bodySmall.copyWith(color: colors.text),
                        ),
                    ],
                  ),
                ),
            ],
          ],
        ),
      ),
    );
  }
}
