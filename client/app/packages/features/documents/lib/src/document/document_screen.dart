import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_documents/src/common/custody.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/labels.dart';
import 'package:strata_documents/src/common/record_move.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A document page (PLAN §11 screen 7a): properties, where it is now (place
/// breadcrumb, holder / last holder), custody history with citations, copies,
/// expiry, "Record a move", and the user's notes. Compact: full-screen page
/// with its own bar; medium: page + context drawer; expanded: page + context
/// panel (graph, concerns).
class DocumentScreen extends StatelessWidget {
  /// Creates the page of [documentId].
  const new(
    this.documentId, {
    super.key,
    this.onOpenEntity,
    this.onOpenNote,
    this.onOpenMindMap,
    this.onBack,
  });

  /// The document's note ID.
  final String documentId;

  /// Opens a person, company, document or place page.
  final ValueChanged<String>? onOpenEntity;

  /// Opens a note at a block (citations, "Open note").
  final OpenNoteAt? onOpenNote;

  /// Opens a local mind map.
  final ValueChanged<String>? onOpenMindMap;

  /// Back to the list (compact bar; breadcrumb on wider windows).
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) => DocumentsLocalizationScope(
    child: EntityLinksScope(
      links: EntityLinks(
        onOpenEntity: onOpenEntity,
        onOpenNote: onOpenNote,
        onOpenMindMap: onOpenMindMap,
        onBack: onBack,
      ),
      child: _DocumentPage(documentId: documentId),
    ),
  );
}

class _DocumentPage extends ConsumerWidget {
  const new({required this.documentId});

  final String documentId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return switch (ref.watch(entityProvider(documentId))) {
      AsyncData(:final value) => switch (value.document) {
        final DocumentView document => DocumentPage(document: document),
        null => const PageNotFound(),
      },
      AsyncError(:final error) => PageError(error: error),
      _ => const PageLoading(),
    };
  }
}

/// The document page body for a [DocumentView] (also embedded by the
/// directory's expanded Documents tab).
class DocumentPage extends StatelessWidget {
  /// Creates the page.
  const new({required this.document, super.key});

  /// The document.
  final DocumentView document;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final links = EntityLinks.of(context);
    final compact = SizeClass.of(context) == SizeClass.compact;
    final docType = document.docType;
    final copy = document.copy;
    final expires = document.expires;
    final openNote = links.onOpenNote;
    void recordMove() => openRecordMove(
      context,
      document: EntityRef(id: document.id, title: document.title),
      location: document.location,
    );

    final typeLine = switch ((docType, copy)) {
      (final String t, final String c) => l10n.docSubtitle(
        type: documentTypeLabel(l10n, t),
        copy: copyLabel(l10n, c),
      ),
      (final String t, null) => documentTypeLabel(l10n, t),
      (null, final String c) => copyLabel(l10n, c),
      (null, null) => l10n.documentKind,
    };

    final header = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          KindAvatar(kind: NodeKind.document, size: compact ? 44 : 52),
          const SizedBox(width: StrataSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Wrap(
                  spacing: StrataSpacing.s2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Semantics(
                      header: true,
                      child: Text(
                        document.title,
                        style: compact ? text.title : text.display,
                      ),
                    ),
                    for (final alias in document.aliases.take(1))
                      Text(
                        alias,
                        style: text.body.copyWith(color: colors.text2),
                      ),
                  ],
                ),
                Wrap(
                  spacing: StrataSpacing.s2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      typeLine,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                    for (final concern in document.concerns)
                      EntityLink(concern),
                  ],
                ),
                if (expires != null && compact)
                  Text(
                    l10n.expiresOn(date: expires),
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
              ],
            ),
          ),
        ],
      ),
    );

    final properties = compact
        ? null
        : Padding(
            padding: const EdgeInsets.only(top: StrataSpacing.s4),
            child: Wrap(
              spacing: StrataSpacing.s4,
              runSpacing: StrataSpacing.s3,
              children: [
                _Property(label: l10n.typeLabel, value: Text(typeLine)),
                if (document.concerns.isNotEmpty)
                  _Property(
                    label: l10n.concernsLabel,
                    value: Wrap(
                      spacing: StrataSpacing.s2,
                      children: [
                        for (final concern in document.concerns)
                          EntityLink(concern),
                      ],
                    ),
                  ),
                _Property(
                  label: l10n.copiesLabel,
                  value: Text(l10n.copiesCount(count: document.copies.length)),
                ),
                if (expires != null)
                  _Property(
                    label: l10n.expiresLabel,
                    value: Text(l10n.dateShort(date: expires)),
                  ),
                _Property(
                  label: l10n.renewalLabel,
                  value: Text(
                    l10n.notAvailableYet,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                ),
              ],
            ),
          );

    final main = <Widget>[
      header,
      ?properties,
      const SizedBox(height: StrataSpacing.s4),
      WhereItIsCard(document: document, onRecordMove: recordMove),
      PageSection(
        title: l10n.custodyHistory,
        caption: compact
            ? l10n.custodyCount(count: document.custody.length)
            : l10n.custodyHint,
        children: [CustodyList(document.custody)],
      ),
      PageSection(
        title: l10n.copiesLabel,
        caption: l10n.copiesCount(count: document.copies.length),
        children: [
          for (final copyRef in document.copies)
            Card(
              child: Padding(
                padding: const EdgeInsets.all(StrataSpacing.s3),
                child: Row(
                  children: [
                    const KindAvatar(kind: NodeKind.document, size: 36),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(child: EntityLink(copyRef)),
                  ],
                ),
              ),
            ),
        ],
      ),
      PageSection(
        title: l10n.yourNotes,
        caption: l10n.yourNotesHint,
        children: [
          Text(
            l10n.yourNotesUnavailable,
            style: text.bodySmall.copyWith(color: colors.text2),
          ),
        ],
      ),
    ];

    final contextPanel = <Widget>[
      MiniGraph(document.id, onOpenMindMap: links.onOpenMindMap),
      if (document.concerns.isNotEmpty)
        PageSection(
          title: l10n.concernsLabel,
          children: [
            for (final concern in document.concerns)
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: EntityLink(concern),
              ),
          ],
        ),
    ];

    return DetailLayout(
      sectionLabel: l10n.documents,
      backLabel: l10n.backToDocuments,
      title: document.title,
      actions: [
        if (!compact)
          FilledButton.tonal(
            onPressed: recordMove,
            child: Text(l10n.recordMove),
          ),
      ],
      menu: [
        if (openNote != null)
          PopupMenuItem(
            value: () => openNote(document.id, null),
            child: Text(l10n.openNote),
          ),
      ],
      main: main,
      contextPanel: compact ? const [] : contextPanel,
    );
  }
}

class _Property extends StatelessWidget {
  const new({required this.label, required this.value});

  final String label;
  final Widget value;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return SizedBox(
      width: 200,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: text.caption.copyWith(color: colors.text2)),
          DefaultTextStyle.merge(style: text.bodySmall, child: value),
        ],
      ),
    );
  }
}
