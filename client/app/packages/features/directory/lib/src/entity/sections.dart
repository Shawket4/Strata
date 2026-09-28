import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/entity_picker.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_state/strata_state.dart' as vm show RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// An entity's avatar: the core's initials in a circle (people) or a
/// rounded square (companies); documents, places and entities without
/// initials show the kind's glyph.
class EntityAvatar extends StatelessWidget {
  /// Creates the avatar for an entity of [kind].
  const new({
    required this.kind,
    super.key,
    this.initials = '',
    this.size = 40,
  });

  /// The node kind.
  final NodeKind kind;

  /// Initials from the core (`initials`; empty: the glyph).
  final String initials;

  /// Diameter.
  final double size;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    if (initials.isNotEmpty &&
        (kind == NodeKind.person || kind == NodeKind.company)) {
      return StrataAvatar(
        initials: initials,
        size: size,
        square: kind == NodeKind.company,
      );
    }
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
                      Text(
                        bullet.text,
                        textDirection: textDirectionOf(bullet.dir),
                        textAlign: TextAlign.start,
                        style: text.bodySmall,
                      ),
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
                    entry.dateLabel ?? '',
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
      Text(
        entry.text,
        textDirection: textDirectionOf(entry.dir),
        textAlign: TextAlign.start,
        style: context.strataText.bodySmall,
      ),
      if (entry.citations.isNotEmpty) CitationRow(entry.citations),
    ],
  );
}

/// Notes mentioning the entity, newest first: title, the core's date label
/// and the snippet with the mention highlighted (`highlights`, UTF-16).
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
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Expanded(
                            child: Text(
                              note.title,
                              textDirection: textDirectionOf(note.titleDir),
                              textAlign: TextAlign.start,
                              style: text.bodySmall.withWeight(
                                FontWeight.w600,
                              ),
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

/// The glyph kind of a directory tab's rows.
NodeKind tabKind(DirectoryTab tab) => switch (tab) {
  DirectoryTab.people => NodeKind.person,
  DirectoryTab.companies => NodeKind.company,
  DirectoryTab.documents => NodeKind.document,
  DirectoryTab.places => NodeKind.place,
};

/// The node kind of the core's entity kind string.
NodeKind entityKindOf(String kind) => switch (kind) {
  'company' => NodeKind.company,
  'document' => NodeKind.document,
  'place' => NodeKind.place,
  _ => NodeKind.person,
};

/// The directory tab listing entities of [kind] (`person`, `company`, …).
DirectoryTab tabOfKind(String? kind) => switch (kind) {
  'company' => DirectoryTab.companies,
  'document' => DirectoryTab.documents,
  'place' => DirectoryTab.places,
  _ => DirectoryTab.people,
};

/// Entity-to-entity relations: the core's relation label, the target, the
/// AI tag with its reason and citations; Reject (an AI link is recorded as
/// rejected and never re-proposed, D13; a user link is removed) and Repoint
/// ("this Ahmed is Ahmed Fathy": pick another entity).
class RelatedEntities extends ConsumerWidget {
  /// Creates the list for the entity [entityId].
  const new({required this.entityId, required this.related, super.key});

  /// The entity whose relations these are.
  final String entityId;

  /// Relations (both directions).
  final List<vm.RelationChip> related;

  Future<void> _run(
    BuildContext context,
    Future<String> Function() intent,
  ) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final l10n = context.dirL10n;
    try {
      await intent();
    } on Object catch (error) {
      messenger
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(dirFailure(l10n, error))));
    }
  }

  Future<void> _repoint(
    BuildContext context,
    WidgetRef ref,
    vm.RelationChip relation,
  ) async {
    final l10n = context.dirL10n;
    final dstId = relation.target.id;
    if (dstId == null) return;
    final newDst = await showEntityPicker(
      context,
      tab: tabOfKind(relation.target.kind),
      title: l10n.repointTitle(title: relation.target.title),
      excludeId: dstId,
    );
    if (newDst == null || !context.mounted) return;
    await _run(
      context,
      () => ref
          .read(coreApiProvider)
          .repointRelation(
            srcId: entityId,
            dstId: dstId,
            relType: relation.relType,
            newDstId: newDst,
          ),
    );
  }

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
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Wrap(
                  crossAxisAlignment: WrapCrossAlignment.center,
                  spacing: StrataSpacing.s2,
                  children: [
                    Text(
                      relation.relLabel,
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
                    if (relation.target.id != null) ...[
                      IconButton(
                        tooltip: l10n.rejectRelation(
                          type: relation.relLabel,
                          title: relation.target.title,
                        ),
                        onPressed: () => _run(
                          context,
                          () => relation.by == 'ai'
                              ? ref
                                    .read(coreApiProvider)
                                    .rejectRelation(
                                      srcId: entityId,
                                      dstId: relation.target.id!,
                                      relType: relation.relType,
                                    )
                              : ref
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
                        tooltip: l10n.repointRelation(
                          title: relation.target.title,
                        ),
                        onPressed: () => _repoint(context, ref, relation),
                        icon: const Icon(Icons.alt_route, size: 18),
                      ),
                    ],
                  ],
                ),
                if (relation.reason case final reason?)
                  Text(
                    reason,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                if (relation.citations.isNotEmpty ||
                    relation.createdLabel != null)
                  CitationRow(
                    relation.citations,
                    trailing: [
                      if (relation.createdLabel case final created?)
                        Text(
                          created,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
              ],
            ),
          ),
      ],
    );
  }
}

/// The message of a failed directory intent.
String dirFailure(DirectoryLocalizations l10n, Object error) =>
    l10n.actionFailed(code: error is CoreFailure ? error.code : 'internal');
