import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/entity_picker.dart';
import 'package:strata_directory/src/entity/property_dialog.dart';
import 'package:strata_directory/src/entity/sections.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// An entity page (PLAN §11 screen 8) for any directory ID: person and
/// company pages here; documents and places render their own pages
/// ([DocumentPage], [PlacePage]) — the core says which kind the ID is.
class EntityPage extends StatelessWidget {
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

  /// Opens another entity page (also the merge target after a merge).
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
/// [EntityPage] and the directory's detail pane).
class EntityDetail extends ConsumerWidget {
  /// Creates the detail of [entityId].
  const new({required this.entityId, super.key});

  /// The entity's note ID.
  final String entityId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return switch (ref.watch(entityProvider(entityId))) {
      AsyncData(:final value) => switch (value.kind) {
        EntityPageKind.entity when value.entity != null => EntityPageBody(
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

/// Runs an entity intent and reports a failure in a snack bar.
Future<bool> runEntityIntent(
  BuildContext context,
  Future<Object?> Function() intent,
) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final l10n = context.dirL10n;
  try {
    await intent();
    return true;
  } on Object catch (error) {
    messenger
      ?..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(dirFailure(l10n, error))));
    return false;
  }
}

/// "Merge…": picks the entity [entity] merges into, shows what moves
/// (`merge_preview`) and merges (`merge_entities`). Returns the target's ID
/// after a merge.
Future<String?> mergeEntity(
  BuildContext context,
  WidgetRef ref,
  EntityView entity,
) async {
  final l10n = context.dirL10n;
  final into = await showEntityPicker(
    context,
    tab: tabOfKind(entity.kind),
    title: l10n.mergeInto(title: entity.title),
    excludeId: entity.id,
  );
  if (into == null || !context.mounted) return null;
  final merged = await showDialog<bool>(
    context: context,
    builder: (_) => DirectoryLocalizationScope(
      child: MergeDialog(sourceId: entity.id, intoId: into),
    ),
  );
  return merged ?? false ? into : null;
}

/// Confirms a merge with the core's preview: which aliases, mentions and
/// relations move to the target.
class MergeDialog extends HookConsumerWidget {
  /// Creates the dialog merging [sourceId] into [intoId].
  const new({required this.sourceId, required this.intoId, super.key});

  /// The entity that goes away.
  final String sourceId;

  /// The entity that stays.
  final String intoId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final busy = useState(false);
    final preview = ref.watch(mergePreviewProvider(sourceId, intoId));
    final value = preview.value;

    Future<void> merge() async {
      busy.value = true;
      final done = await runEntityIntent(
        context,
        () => ref
            .read(coreApiProvider)
            .mergeEntities(sourceId: sourceId, intoId: intoId),
      );
      if (!context.mounted) return;
      busy.value = false;
      if (done) Navigator.of(context).pop(true);
    }

    return AlertDialog(
      title: Text(l10n.mergeTitle),
      content: switch (preview) {
        AsyncError(:final error) => Text(dirFailure(l10n, error)),
        _ when value == null => const SizedBox(
          height: 64,
          child: Center(child: CircularProgressIndicator()),
        ),
        _ => Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.mergeBody(
                source: value.source.title,
                into: value.into.title,
              ),
              style: text.body,
            ),
            const SizedBox(height: StrataSpacing.s3),
            Text(
              l10n.mergeMoves(
                mentions: value.mentionCount,
                relations: value.relationCount,
              ),
              style: text.bodySmall,
            ),
            if (value.aliases.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s2),
              Text(
                l10n.mergeAliases,
                style: text.caption.copyWith(color: colors.text2),
              ),
              Wrap(
                spacing: StrataSpacing.s2,
                runSpacing: StrataSpacing.s1,
                children: [
                  for (final alias in value.aliases) Chip(label: Text(alias)),
                ],
              ),
            ],
          ],
        ),
      },
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: value == null || busy.value ? null : merge,
          child: Text(l10n.mergeConfirm),
        ),
      ],
    );
  }
}

/// A dialog with one text field per label; returns the values, or null
/// when cancelled.
Future<List<String>?> showFieldsDialog(
  BuildContext context, {
  required String title,
  required List<String> labels,
  required String confirm,
  List<String> initial = const [],
}) => showDialog<List<String>>(
  context: context,
  builder: (_) => DirectoryLocalizationScope(
    child: _FieldsDialog(
      title: title,
      labels: labels,
      confirm: confirm,
      initial: initial,
    ),
  ),
);

class _FieldsDialog extends HookWidget {
  const new({
    required this.title,
    required this.labels,
    required this.confirm,
    required this.initial,
  });

  final String title;
  final List<String> labels;
  final String confirm;
  final List<String> initial;

  @override
  Widget build(BuildContext context) {
    final controllers = [
      for (var i = 0; i < labels.length; i++)
        useTextEditingController(text: i < initial.length ? initial[i] : ''),
    ];
    return AlertDialog(
      title: Text(title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var i = 0; i < labels.length; i++)
            Padding(
              padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
              child: TextField(
                controller: controllers[i],
                autofocus: i == 0,
                decoration: InputDecoration(labelText: labels[i]),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(context.dirL10n.cancel),
        ),
        FilledButton(
          onPressed: () =>
              Navigator.of(context).pop([for (final c in controllers) c.text]),
          child: Text(confirm),
        ),
      ],
    );
  }
}

/// A person or company page: header (initials, aliases, mention count, last
/// activity, tags, editable properties and aliases), Summary with its
/// citations, Insights, Open items and Timeline (each bullet cites its
/// source), the user's notes (`## Notes`, editable), and (context)
/// mentioning notes, related entities, documents and the entity graph.
/// Compact: Overview / Notes / Graph tabs.
class EntityPageBody extends ConsumerWidget {
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
    final core = ref.read(coreApiProvider);

    void refresh() {
      unawaited(core.requestRelink(noteId: entity.id));
      ScaffoldMessenger.maybeOf(context)
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(l10n.refreshQueued)));
    }

    Future<void> merge() async {
      final into = await mergeEntity(context, ref, entity);
      if (into == null || !context.mounted) return;
      ScaffoldMessenger.maybeOf(context)
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(l10n.merged)));
      links.onOpenEntity?.call(into);
    }

    Future<void> addAlias() async {
      final values = await showFieldsDialog(
        context,
        title: l10n.addAlias,
        labels: [l10n.aliasField],
        confirm: l10n.add,
      );
      if (values == null || !context.mounted) return;
      await runEntityIntent(
        context,
        () => core.addAlias(id: entity.id, alias: values.first),
      );
    }

    Future<void> editProperty([PropertyItem? property]) async {
      final edit = await showPropertyDialog(
        context,
        title: property == null
            ? l10n.addProperty
            : l10n.editProperty(key: property.key),
        key: property?.key ?? '',
        values: property?.values ?? const [],
      );
      if (edit == null || !context.mounted) return;
      if (property != null && property.key != edit.key) {
        final removed = await runEntityIntent(
          context,
          () => core.removeProperty(id: entity.id, key: property.key),
        );
        if (!removed || !context.mounted) return;
      }
      // Every value, as a list: the core writes one value as a scalar.
      await runEntityIntent(
        context,
        () => core.setPropertyValues(
          id: entity.id,
          key: edit.key,
          values: edit.values,
        ),
      );
    }

    final mentionLine = l10n.entitySubtitle(
      kind: kindLabel,
      count: entity.mentionCount,
    );
    final header = Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EntityAvatar(
            kind: kind,
            initials: entity.initials,
            size: compact ? 52 : 60,
          ),
          const SizedBox(width: StrataSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Semantics(
                  header: true,
                  child: Text(
                    entity.title,
                    textDirection: textDirectionOf(entity.titleDir),
                    style: compact ? text.title : text.display,
                  ),
                ),
                Text(
                  entity.lastActiveLabel == null
                      ? mentionLine
                      : l10n.entitySubtitleActive(
                          subtitle: mentionLine,
                          when: entity.lastActiveLabel!,
                        ),
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
                if (entity.tags.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: StrataSpacing.s1),
                    child: Wrap(
                      spacing: StrataSpacing.s1,
                      runSpacing: StrataSpacing.s1,
                      children: [
                        for (final tag in entity.tags)
                          Chip(
                            label: Text(tag),
                            visualDensity: VisualDensity.compact,
                          ),
                      ],
                    ),
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
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          for (final property in entity.properties)
            SizedBox(
              width: compact ? 160 : 200,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
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
                  PopupMenuButton<VoidCallback>(
                    tooltip: l10n.propertyActions(key: property.key),
                    icon: const Icon(Icons.more_horiz, size: 18),
                    onSelected: (action) => action(),
                    itemBuilder: (_) => [
                      if (property.values.length <= 1)
                        PopupMenuItem(
                          value: () => unawaited(editProperty(property)),
                          child: Text(l10n.edit),
                        ),
                      PopupMenuItem(
                        value: () => unawaited(
                          runEntityIntent(
                            context,
                            () => core.removeProperty(
                              id: entity.id,
                              key: property.key,
                            ),
                          ),
                        ),
                        child: Text(l10n.remove),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          TextButton.icon(
            onPressed: () => unawaited(editProperty()),
            icon: const Icon(Icons.add, size: 18),
            label: Text(l10n.addProperty),
          ),
        ],
      ),
    );

    final aliases = Padding(
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
            InputChip(
              label: Text(alias),
              deleteButtonTooltipMessage: l10n.removeAlias(alias: alias),
              onDeleted: () => unawaited(
                runEntityIntent(
                  context,
                  () => core.removeAlias(id: entity.id, alias: alias),
                ),
              ),
            ),
          TextButton.icon(
            onPressed: () => unawaited(addAlias()),
            icon: const Icon(Icons.add, size: 18),
            label: Text(l10n.addAlias),
          ),
        ],
      ),
    );

    final summary = entity.summary;
    final summarySection = PageSection(
      title: l10n.summary,
      caption: entity.aiUpdatedLabel == null
          ? l10n.aiMaintained
          : l10n.aiUpdated(when: entity.aiUpdatedLabel!),
      children: [
        Text(
          summary ?? l10n.noSummary,
          textDirection: summary == null
              ? null
              : textDirectionOf(entity.summaryDir),
          textAlign: TextAlign.start,
          style: summary == null
              ? text.bodySmall.copyWith(color: colors.text2)
              : text.body,
        ),
        if (entity.summaryCitations.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: StrataSpacing.s1),
            child: CitationRow(entity.summaryCitations),
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
      caption: l10n.openDone(open: entity.openCount, done: entity.doneCount),
      children: [CitedBullets(entity.openItems, openItem: true)],
    );
    final timeline = PageSection(
      title: l10n.timeline,
      caption: l10n.aiMaintained,
      children: [TimelineList(entity.timeline)],
    );
    final yourNotes = UserNotesSection(
      id: entity.id,
      text: entity.userNotes,
      title: l10n.yourNotes,
      caption: l10n.yourNotesHint,
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
      properties,
      aliases,
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
      PopupMenuItem(value: () => unawaited(merge()), child: Text(l10n.merge)),
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
                  Tab(text: l10n.tabNotes(count: entity.mentionCount)),
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
      subtitle: entity.path.isEmpty ? null : entity.path,
      actions: [
        if (!compact) ...[
          OutlinedButton(onPressed: refresh, child: Text(l10n.refreshInsights)),
          const SizedBox(width: StrataSpacing.s2),
          OutlinedButton(
            onPressed: () => unawaited(merge()),
            child: Text(l10n.merge),
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
