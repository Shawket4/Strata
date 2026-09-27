import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_state/strata_state.dart' as vm show RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A circular avatar with the entity kind's glyph (initials need a core
/// field, see CORE_GAPS).
class EntityAvatar extends StatelessWidget {
  /// Creates the avatar for an entity of [kind].
  const new({required this.kind, super.key, this.size = 40});

  /// `person` / `company` / … (the core's kind string).
  final NodeKind kind;

  /// Diameter.
  final double size;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: kind == NodeKind.person || kind == NodeKind.company
            ? colors.surface2
            : colors.accentTint,
        shape: kind == NodeKind.person ? BoxShape.circle : BoxShape.rectangle,
        borderRadius: kind == NodeKind.person
            ? null
            : BorderRadius.circular(size * 0.25),
        border: Border.all(color: colors.border),
      ),
      child: NodeKindGlyph(kind: kind, size: size * 0.5, decorative: true),
    );
  }
}

/// A list of AI bullets (Insights, Open items), each with its citations.
class CitedBullets extends StatelessWidget {
  /// Creates the list.
  const new(this.bullets, {super.key, this.openItem = false});

  /// Bullets in the core's order.
  final List<CitedBullet> bullets;

  /// Open items get a hollow marker.
  final bool openItem;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    if (bullets.isEmpty) {
      return Text(
        context.dirL10n.nothingYet,
        style: text.bodySmall.copyWith(color: colors.text2),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final bullet in bullets)
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(top: 7),
                  child: Icon(
                    openItem ? Icons.radio_button_unchecked : Icons.circle,
                    size: openItem ? 14 : 6,
                    color: colors.text2,
                  ),
                ),
                const SizedBox(width: StrataSpacing.s2),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(bullet.text, style: text.bodySmall),
                      if (bullet.citations.isNotEmpty)
                        CitationRow(bullet.citations),
                    ],
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// The dated Timeline, newest first (the core's order).
class TimelineList extends StatelessWidget {
  /// Creates the timeline.
  const new(this.entries, {super.key});

  /// Entries.
  final List<CitedBullet> entries;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    if (entries.isEmpty) {
      return Text(
        l10n.nothingYet,
        style: text.bodySmall.copyWith(color: colors.text2),
      );
    }
    final wide = SizeClass.of(context) != SizeClass.compact;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final entry in entries)
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
            child: Flex(
              direction: wide ? Axis.horizontal : Axis.vertical,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(
                  width: wide ? 132 : null,
                  child: Text(
                    entry.date == null ? '' : l10n.dateShort(date: entry.date!),
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                ),
                if (wide)
                  Expanded(child: _entryBody(context, entry))
                else
                  _entryBody(context, entry),
              ],
            ),
          ),
      ],
    );
  }

  Widget _entryBody(BuildContext context, CitedBullet entry) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(entry.text, style: context.strataText.bodySmall),
      if (entry.citations.isNotEmpty) CitationRow(entry.citations),
    ],
  );
}

/// Notes mentioning the entity, newest first.
class MentionsList extends StatelessWidget {
  /// Creates the list.
  const new(this.mentions, {super.key});

  /// The notes.
  final List<NoteListItem> mentions;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
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
                      Wrap(
                        spacing: StrataSpacing.s2,
                        alignment: WrapAlignment.spaceBetween,
                        children: [
                          Text(
                            note.title,
                            style: text.bodySmall.withWeight(FontWeight.w600),
                          ),
                          Text(
                            l10n.mentionDate(date: note.updatedAt),
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                        ],
                      ),
                      if (note.snippet.isNotEmpty)
                        Text(
                          note.snippet,
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          textAlign: TextAlign.start,
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

/// Entity-to-entity relations with the AI tag, Reject (D13: removes the AI
/// link through the core) and Repoint (needs a core intent: disabled).
class RelatedEntities extends ConsumerWidget {
  /// Creates the list for the entity [entityId].
  const new({required this.entityId, required this.related, super.key});

  /// The entity whose relations these are.
  final String entityId;

  /// Relations (both directions).
  final List<vm.RelationChip> related;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    if (related.isEmpty) {
      return Text(
        l10n.noRelated,
        style: text.bodySmall.copyWith(color: colors.text2),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final relation in related)
          Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: StrataSpacing.s2,
            children: [
              Text(
                relation.relType,
                style: text.caption.copyWith(color: colors.text2),
              ),
              EntityLink(relation.target),
              if (relation.by == 'ai' && relation.confidence != null)
                StatusPill(
                  label: l10n.aiConfidence(
                    value: relation.confidence!.toStringAsFixed(2),
                  ),
                  tone: StatusTone.info,
                  icon: Icons.auto_awesome_outlined,
                ),
              if (relation.target.id != null)
                IconButton(
                  tooltip: l10n.rejectRelation(
                    type: relation.relType,
                    title: relation.target.title,
                  ),
                  onPressed: () => unawaited(
                    ref
                        .read(coreApiProvider)
                        .removeRelation(
                          srcId: entityId,
                          dstId: relation.target.id!,
                          relType: relation.relType,
                        ),
                  ),
                  icon: const Icon(Icons.link_off, size: 18),
                ),
              IconButton(
                tooltip: l10n.repointUnavailable,
                onPressed: null,
                icon: const Icon(Icons.alt_route, size: 18),
              ),
            ],
          ),
      ],
    );
  }
}
