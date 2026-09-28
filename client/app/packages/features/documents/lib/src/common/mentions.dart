import 'package:flutter/material.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Notes mentioning the entity, newest first: title, the core's date label
/// and the snippet with the mention highlighted (`highlights`, UTF-16).
class MentionsList extends StatelessWidget {
  /// Creates the list.
  const new(this.mentions, {super.key});

  /// The notes.
  final List<NoteListItem> mentions;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = EntityLinks.of(context).onOpenNote;
    if (mentions.isEmpty) {
      return Text(
        l10n.noMentions,
        style: text.bodySmall.copyWith(color: colors.text2),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final note in mentions)
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
            child: Card(
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                onTap: open == null ? null : () => open(note.id, null),
                child: Padding(
                  padding: const EdgeInsets.all(StrataSpacing.s3),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Expanded(
                            child: Text(
                              note.title,
                              textDirection: textDirectionOf(note.titleDir),
                              textAlign: TextAlign.start,
                              style: text.bodySmall.withWeight(FontWeight.w600),
                            ),
                          ),
                          const SizedBox(width: StrataSpacing.s2),
                          Text(
                            note.updatedLabel,
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                        ],
                      ),
                      if (note.snippet.isNotEmpty)
                        StrataHighlightedText(
                          note.snippet,
                          highlights: textRangesOf(note.highlights),
                          maxLines: 3,
                          textDirection: textDirectionOf(note.snippetDir),
                          style: text.bodySmall.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}
