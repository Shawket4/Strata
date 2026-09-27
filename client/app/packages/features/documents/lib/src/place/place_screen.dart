import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_documents/src/common/custody.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/record_move.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A place page (PLAN §11 screen 7b): enclosing places, the places inside
/// it, everything stored here including nested places, recent movements and
/// "Record a move". Compact: one column with a pinned "Record a move";
/// expanded: places tree beside the documents table, context panel.
class PlaceScreen extends StatelessWidget {
  /// Creates the page of [placeId].
  const new(
    this.placeId, {
    super.key,
    this.onOpenEntity,
    this.onOpenNote,
    this.onOpenMindMap,
    this.onBack,
  });

  /// The place's note ID.
  final String placeId;

  /// Opens a person, company, document or place page.
  final ValueChanged<String>? onOpenEntity;

  /// Opens a note at a block.
  final OpenNoteAt? onOpenNote;

  /// Opens a local mind map.
  final ValueChanged<String>? onOpenMindMap;

  /// Back to the list.
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
      child: _PlaceLoader(placeId: placeId),
    ),
  );
}

class _PlaceLoader extends ConsumerWidget {
  const new({required this.placeId});

  final String placeId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return switch (ref.watch(entityProvider(placeId))) {
      AsyncData(:final value) => switch (value.place) {
        final PlaceView place => PlacePage(place: place),
        null => const PageNotFound(),
      },
      AsyncError(:final error) => PageError(error: error),
      _ => const PageLoading(),
    };
  }
}

/// The place page body for a [PlaceView] (also embedded by the directory's
/// expanded Places tab).
class PlacePage extends StatelessWidget {
  /// Creates the page.
  const new({required this.place, super.key});

  /// The place.
  final PlaceView place;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final links = EntityLinks.of(context);
    final sizeClass = SizeClass.of(context);
    final compact = sizeClass == SizeClass.compact;
    final openNote = links.onOpenNote;
    void recordMove() => openRecordMove(context, documents: place.documents);
    final parent = place.breadcrumb.isEmpty ? null : place.breadcrumb.last;

    final header = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (place.breadcrumb.isNotEmpty)
            PlaceBreadcrumb([
              ...place.breadcrumb,
              EntityRef(title: place.title),
            ], style: text.bodySmall),
          const SizedBox(height: StrataSpacing.s2),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              KindAvatar(kind: NodeKind.place, size: compact ? 48 : 56),
              const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Semantics(
                      header: true,
                      child: Text(
                        place.title,
                        style: compact ? text.title : text.display,
                      ),
                    ),
                    for (final alias in place.aliases.take(1))
                      Text(
                        alias,
                        style: text.body.copyWith(color: colors.text2),
                      ),
                    Wrap(
                      spacing: StrataSpacing.s1,
                      children: [
                        for (final part in [
                          l10n.placeKind,
                          if (parent == null)
                            l10n.topLevelPlace
                          else
                            l10n.partOf(place: parent.title),
                          l10n.documentsCount(count: place.documents.length),
                        ]) ...[
                          if (part != l10n.placeKind)
                            ExcludeSemantics(
                              child: Text(
                                '·',
                                style: text.bodySmall.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            ),
                          Text(
                            part,
                            style: text.bodySmall.copyWith(color: colors.text2),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
    );

    final tree = PageSection(
      title: l10n.placesTree,
      children: [
        Semantics(
          label: l10n.placesTreeLabel(place: place.title),
          container: true,
          explicitChildNodes: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: StrataSpacing.s3,
                  vertical: StrataSpacing.s2,
                ),
                decoration: BoxDecoration(
                  color: colors.accentTint,
                  borderRadius: StrataRadii.inputRadius,
                ),
                child: Row(
                  children: [
                    const NodeKindGlyph(kind: NodeKind.place, decorative: true),
                    const SizedBox(width: StrataSpacing.s2),
                    Expanded(
                      child: Text(
                        place.title,
                        style: text.bodySmall.withWeight(FontWeight.w600),
                      ),
                    ),
                  ],
                ),
              ),
              if (place.subPlaces.isEmpty)
                Padding(
                  padding: const EdgeInsetsDirectional.only(
                    start: StrataSpacing.s8,
                    top: StrataSpacing.s2,
                  ),
                  child: Text(
                    l10n.noSubPlaces,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                ),
              for (final sub in place.subPlaces)
                Padding(
                  padding: const EdgeInsetsDirectional.only(
                    start: StrataSpacing.s6,
                  ),
                  child: Row(
                    children: [
                      const NodeKindGlyph(
                        kind: NodeKind.place,
                        decorative: true,
                        size: 14,
                      ),
                      const SizedBox(width: StrataSpacing.s1),
                      Flexible(child: EntityLink(sub)),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ],
    );

    final everything = PageSection(
      title: l10n.everythingHere,
      caption: l10n.documentsCount(count: place.documents.length),
      children: [
        if (place.documents.isEmpty)
          Text(
            l10n.noDocumentsHere,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else if (compact)
          Card(
            clipBehavior: Clip.antiAlias,
            child: Column(
              children: [
                for (var i = 0; i < place.documents.length; i++) ...[
                  if (i > 0) Divider(height: 1, color: colors.border),
                  DocumentBriefTile(place.documents[i]),
                ],
              ],
            ),
          )
        else
          _DocumentsTable(place: place),
      ],
    );

    final movements = PageSection(
      title: l10n.recentMovements,
      children: [
        CustodyList(
          place.recentMovements,
          showDocument: true,
          empty: l10n.noMovements,
        ),
      ],
    );

    final List<Widget> main;
    if (sizeClass == SizeClass.expanded) {
      main = [
        header,
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(width: 268, child: tree),
            const SizedBox(width: StrataSpacing.s6),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [everything, movements],
              ),
            ),
          ],
        ),
      ];
    } else {
      main = [header, everything, tree, movements];
    }

    return DetailLayout(
      sectionLabel: l10n.places,
      backLabel: l10n.backToPlaces,
      title: place.title,
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
            value: () => openNote(place.id, null),
            child: Text(l10n.openNote),
          ),
      ],
      main: main,
      contextPanel: compact
          ? const []
          : [
              MiniGraph(place.id, onOpenMindMap: links.onOpenMindMap),
              if (place.aliases.isNotEmpty)
                PageSection(
                  title: l10n.aliases,
                  children: [
                    Wrap(
                      spacing: StrataSpacing.s2,
                      runSpacing: StrataSpacing.s2,
                      children: [
                        for (final alias in place.aliases)
                          Chip(label: Text(alias)),
                      ],
                    ),
                  ],
                ),
            ],
      compactFooter: compact
          ? FilledButton.icon(
              onPressed: recordMove,
              icon: const Icon(Icons.swap_horiz, size: 20),
              label: Text(l10n.recordMove),
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(48),
              ),
            )
          : null,
    );
  }
}

class _DocumentsTable extends StatelessWidget {
  const new({required this.place});

  final PlaceView place;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = EntityLinks.of(context).onOpenEntity;
    final head = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2);
    Widget row(List<Widget> cells, {VoidCallback? onTap, String? label}) {
      final content = Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s3,
          vertical: StrataSpacing.s2,
        ),
        child: Row(
          children: [
            Expanded(flex: 38, child: cells[0]),
            Expanded(flex: 18, child: cells[1]),
            Expanded(flex: 24, child: cells[2]),
            Expanded(
              flex: 20,
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: cells[3],
              ),
            ),
          ],
        ),
      );
      if (onTap == null) return content;
      return Semantics(
        button: true,
        label: label,
        child: InkWell(onTap: onTap, child: content),
      );
    }

    return Semantics(
      label: l10n.everythingHereCaption(place: place.title),
      container: true,
      explicitChildNodes: true,
      child: Card(
        clipBehavior: Clip.antiAlias,
        child: Column(
          children: [
            ColoredBox(
              color: colors.background,
              child: row([
                Text(l10n.colDocument, style: head),
                Text(l10n.colWhere, style: head),
                Text(l10n.colHolder, style: head),
                Text(l10n.colStatus, style: head),
              ]),
            ),
            for (final document in place.documents) ...[
              Divider(height: 1, color: colors.border),
              row(
                [
                  Text(
                    document.title,
                    style: text.bodySmall.withWeight(FontWeight.w600),
                  ),
                  Text(document.location?.title ?? '', style: text.bodySmall),
                  Text(
                    document.holder?.title ?? l10n.nobody,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                  if (document.status == null)
                    const SizedBox()
                  else
                    DocumentStatusPill(document.status!),
                ],
                label: document.title,
                onTap: open == null ? null : () => open(document.id),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
