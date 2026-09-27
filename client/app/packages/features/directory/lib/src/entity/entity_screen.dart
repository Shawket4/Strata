import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/sections.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// An entity page (PLAN §11 screen 8) for any directory ID: person and
/// company pages here; documents and places render their own pages
/// ([DocumentPage], [PlacePage]) — the core says which kind the ID is.
class EntityScreen extends StatelessWidget {
  /// Creates the page of [entityId].
  const new(
    this.entityId, {
    super.key,
    this.onOpenEntity,
    this.onOpenNote,
    this.onOpenMindMap,
    this.onBack,
  });

  /// The entity's note ID.
  final String entityId;

  /// Opens another entity page.
  final ValueChanged<String>? onOpenEntity;

  /// Opens a note at a block (citations, mentions).
  final OpenNoteAt? onOpenNote;

  /// Opens a local mind map.
  final ValueChanged<String>? onOpenMindMap;

  /// Back to the list.
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) => DirectoryLocalizationScope(
    child: DocumentsLocalizationScope(
      child: EntityLinksScope(
        links: EntityLinks(
          onOpenEntity: onOpenEntity,
          onOpenNote: onOpenNote,
          onOpenMindMap: onOpenMindMap,
          onBack: onBack,
        ),
        child: EntityDetail(entityId: entityId),
      ),
    ),
  );
}

/// Loads [entityId] and renders the page for its kind (used by
/// [EntityScreen] and the directory's detail pane).
class EntityDetail extends ConsumerWidget {
  /// Creates the detail of [entityId].
  const new({required this.entityId, super.key});

  /// The entity's note ID.
  final String entityId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return switch (ref.watch(entityProvider(entityId))) {
      AsyncData(:final value) => switch (value.kind) {
        EntityPageKind.entity when value.entity != null => EntityPage(
          entity: value.entity!,
        ),
        EntityPageKind.document when value.document != null => DocumentPage(
          document: value.document!,
        ),
        EntityPageKind.place when value.place != null => PlacePage(
          place: value.place!,
        ),
        _ => const PageNotFound(),
      },
      AsyncError(:final error) => PageError(error: error),
      _ => const PageLoading(),
    };
  }
}

/// The node kind of an entity kind string.
NodeKind entityKindOf(String kind) =>
    kind == 'company' ? NodeKind.company : NodeKind.person;

/// A person or company page: header with properties and aliases, Summary,
/// Insights, Open items and Timeline (each bullet cites its source), the
/// user's notes, and (context) mentioning notes, related entities, documents
/// and the entity graph. Compact: Overview / Notes / Graph tabs.
class EntityPage extends ConsumerWidget {
  /// Creates the page.
  const new({required this.entity, super.key});

  /// The entity.
  final EntityView entity;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final links = EntityLinks.of(context);
    final sizeClass = SizeClass.of(context);
    final compact = sizeClass == SizeClass.compact;
    final kind = entityKindOf(entity.kind);
    final kindLabel = kind == NodeKind.company
        ? l10n.kindCompany
        : l10n.kindPerson;
    final openNote = links.onOpenNote;

    void refresh() {
      unawaited(ref.read(coreApiProvider).requestRelink(noteId: entity.id));
      ScaffoldMessenger.maybeOf(context)
          ?.showSnackBar(SnackBar(content: Text(l10n.refreshQueued)));
    }

    final header = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EntityAvatar(kind: kind, size: compact ? 52 : 60),
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
                        entity.title,
                        style: compact ? text.title : text.display,
                      ),
                    ),
                    for (final alias in entity.aliases.take(1))
                      Text(
                        alias,
                        style: text.body.copyWith(color: colors.text2),
                      ),
                  ],
                ),
                Text(
                  l10n.entitySubtitle(
                    kind: kindLabel,
                    count: entity.mentions.length,
                  ),
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
                if (entity.pendingSync)
                  Padding(
                    padding: const EdgeInsets.only(top: StrataSpacing.s1),
                    child: StatusPill(
                      label: l10n.pendingSync,
                      tone: StatusTone.warning,
                      icon: Icons.cloud_upload_outlined,
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );

    final properties = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s4),
      child: Wrap(
        spacing: StrataSpacing.s4,
        runSpacing: StrataSpacing.s3,
        children: [
          for (final property in entity.properties)
            SizedBox(
              width: compact ? 160 : 200,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    property.key,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                  for (final value in property.values)
                    Text(value, style: text.bodySmall),
                ],
              ),
            ),
        ],
      ),
    );

    final aliases = entity.aliases.isEmpty
        ? null
        : Padding(
            padding: const EdgeInsets.only(top: StrataSpacing.s3),
            child: Wrap(
              spacing: StrataSpacing.s2,
              runSpacing: StrataSpacing.s1,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  l10n.colAliases,
                  style: text.caption.copyWith(color: colors.text2),
                ),
                for (final alias in entity.aliases)
                  Chip(
                    label: Text(alias),
                    visualDensity: VisualDensity.compact,
                  ),
              ],
            ),
          );

    final summary = entity.summary;
    final summarySection = PageSection(
      title: l10n.summary,
      caption: l10n.aiMaintained,
      children: [
        Text(
          summary ?? l10n.noSummary,
          style: summary == null
              ? text.bodySmall.copyWith(color: colors.text2)
              : text.body,
        ),
      ],
    );
    final insights = PageSection(
      title: l10n.insights,
      caption: l10n.aiMaintained,
      children: [CitedBullets(entity.insights)],
    );
    final openItems = PageSection(
      title: l10n.openItems,
      caption: l10n.aiMaintained,
      children: [CitedBullets(entity.openItems, openItem: true)],
    );
    final timeline = PageSection(
      title: l10n.timeline,
      caption: l10n.aiMaintained,
      children: [TimelineList(entity.timeline)],
    );
    final yourNotes = PageSection(
      title: l10n.yourNotes,
      caption: l10n.yourNotesHint,
      children: [
        Text(
          l10n.yourNotesUnavailable,
          style: text.bodySmall.copyWith(color: colors.text2),
        ),
      ],
    );
    final related = PageSection(
      title: l10n.relatedEntities,
      children: [RelatedEntities(entityId: entity.id, related: entity.related)],
    );
    final mentions = PageSection(
      title: l10n.mentioningNotes,
      caption: l10n.newestFirst,
      children: [MentionsList(entity.mentions)],
    );
    final documents = entity.documents.isEmpty
        ? null
        : PageSection(
            title: l10n.documentsSection,
            children: [
              Card(
                clipBehavior: Clip.antiAlias,
                child: Column(
                  children: [
                    for (final document in entity.documents)
                      DocumentBriefTile(document, dense: true),
                  ],
                ),
              ),
            ],
          );
    final graph = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s6),
      child: MiniGraph(entity.id, onOpenMindMap: links.onOpenMindMap),
    );

    final main = <Widget>[
      header,
      if (entity.properties.isNotEmpty) properties,
      ?aliases,
      summarySection,
      if (sizeClass == SizeClass.expanded)
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: insights),
            const SizedBox(width: StrataSpacing.s6),
            Expanded(child: openItems),
          ],
        )
      else ...[
        insights,
        openItems,
      ],
      timeline,
      if (compact) related,
      ?documents,
      yourNotes,
    ];

    final menu = <PopupMenuEntry<VoidCallback>>[
      PopupMenuItem(value: refresh, child: Text(l10n.refreshInsights)),
      PopupMenuItem(enabled: false, child: Text(l10n.merge)),
      if (openNote != null)
        PopupMenuItem(
          value: () => openNote(entity.id, null),
          child: Text(l10n.openNote),
        ),
    ];

    Widget? compactBody;
    if (compact) {
      const padding = EdgeInsets.fromLTRB(
        StrataSpacing.s4,
        0,
        StrataSpacing.s4,
        StrataSpacing.s8,
      );
      compactBody = DefaultTabController(
        length: 3,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Semantics(
              label: l10n.entityViews,
              container: true,
              explicitChildNodes: true,
              child: TabBar(
                tabs: [
                  Tab(text: l10n.tabOverview),
                  Tab(text: l10n.tabNotes(count: entity.mentions.length)),
                  Tab(text: l10n.tabGraph),
                ],
              ),
            ),
            Expanded(
              child: TabBarView(
                children: [
                  SingleChildScrollView(
                    padding: padding,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: main,
                    ),
                  ),
                  SingleChildScrollView(padding: padding, child: mentions),
                  SingleChildScrollView(
                    padding: const EdgeInsets.only(top: StrataSpacing.s4),
                    child: MiniGraph(
                      entity.id,
                      height: 360,
                      onOpenMindMap: links.onOpenMindMap,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      );
    }

    return DetailLayout(
      sectionLabel: kind == NodeKind.company
          ? l10n.tabCompanies
          : l10n.tabPeople,
      backLabel: kind == NodeKind.company
          ? l10n.backToCompanies
          : l10n.backToPeople,
      title: entity.title,
      actions: [
        if (!compact) ...[
          OutlinedButton(onPressed: refresh, child: Text(l10n.refreshInsights)),
          const SizedBox(width: StrataSpacing.s2),
          Tooltip(
            message: l10n.mergeUnavailable,
            child: OutlinedButton(
              onPressed: null,
              style: OutlinedButton.styleFrom(
                disabledForegroundColor: colors.text2,
              ),
              child: Text(l10n.merge),
            ),
          ),
        ],
      ],
      menu: menu,
      main: compact ? const [] : main,
      compactBody: compactBody,
      contextPanel: compact
          ? const []
          : [
              Padding(
                padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
                child: mentions,
              ),
              related,
              graph,
            ],
    );
  }
}
