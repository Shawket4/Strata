import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/directory/new_entity.dart';
import 'package:strata_directory/src/directory/suggestions.dart';
import 'package:strata_directory/src/entity/entity_screen.dart';
import 'package:strata_directory/src/entity/sections.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The glyph kind of a directory tab's rows.
NodeKind tabKind(DirectoryTab tab) => switch (tab) {
  DirectoryTab.people => NodeKind.person,
  DirectoryTab.companies => NodeKind.company,
  DirectoryTab.documents => NodeKind.document,
  DirectoryTab.places => NodeKind.place,
};

/// The directory (PLAN §11 screen 7): People / Companies / Documents /
/// Places tabs with search in both scripts (the core matches names and
/// aliases), filters, the suggestion strip and rows. Compact: one list, rows
/// open full-screen pages through [onOpenEntity]; medium: list + detail;
/// expanded: people and companies as a dense table with a preview panel,
/// documents and places as list + page (+ its context panel).
class DirectoryScreen extends StatelessWidget {
  /// Creates the directory.
  const new({
    super.key,
    this.initialTab = DirectoryTab.people,
    this.selectedId,
    this.onOpenEntity,
    this.onOpenNote,
    this.onOpenMindMap,
  });

  /// The icon that represents this feature.
  static const IconData icon = Icons.people_alt_outlined;

  /// Tab shown first.
  final DirectoryTab initialTab;

  /// Row selected first (medium and expanded detail / preview).
  final String? selectedId;

  /// Opens an entity page full-screen (compact rows, "Open page").
  final ValueChanged<String>? onOpenEntity;

  /// Opens a note at a block.
  final OpenNoteAt? onOpenNote;

  /// Opens a local mind map.
  final ValueChanged<String>? onOpenMindMap;

  @override
  Widget build(BuildContext context) => DirectoryLocalizationScope(
    child: DocumentsLocalizationScope(
      child: EntityLinksScope(
        links: EntityLinks(
          onOpenEntity: onOpenEntity,
          onOpenNote: onOpenNote,
          onOpenMindMap: onOpenMindMap,
        ),
        child: _Directory(initialTab: initialTab, selectedId: selectedId),
      ),
    ),
  );
}

class _Directory extends HookConsumerWidget {
  const new({required this.initialTab, required this.selectedId});

  final DirectoryTab initialTab;
  final String? selectedId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tab = useState(initialTab);
    final query = useState('');
    final selected = useState(selectedId);
    final async = ref.watch(directoryProvider(tab.value, query.value));
    final view = async.value;
    final sizeClass = SizeClass.of(context);
    final links = EntityLinks.of(context);

    void selectTab(DirectoryTab next) {
      if (next == tab.value) return;
      tab.value = next;
      selected.value = null;
    }

    final tabs = _Tabs(
      tab: tab.value,
      counts: view?.counts,
      onSelected: selectTab,
      dense: sizeClass != SizeClass.compact,
    );
    final search = _SearchField(
      tab: tab.value,
      onChanged: (value) => query.value = value,
    );
    final filters = _Filters(tab: tab.value);
    final Widget list = switch (async) {
      AsyncError(:final error) when view == null => PageError(error: error),
      _ when view == null => const PageLoading(),
      _ => _RowList(
        view: view,
        selected: sizeClass == SizeClass.compact ? null : selected.value,
        onTap: (id) => sizeClass == SizeClass.compact
            ? links.onOpenEntity?.call(id)
            : selected.value = id,
      ),
    };

    if (sizeClass == SizeClass.compact) {
      return ColoredBox(
        color: context.strataColors.background,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            tabs,
            Expanded(
              child: CustomScrollView(
                slivers: [
                  SliverPadding(
                    padding: const EdgeInsets.fromLTRB(
                      StrataSpacing.s4,
                      StrataSpacing.s3,
                      StrataSpacing.s4,
                      0,
                    ),
                    sliver: SliverList.list(
                      children: [search, filters, const SuggestionStrip()],
                    ),
                  ),
                  SliverFillRemaining(hasScrollBody: false, child: list),
                ],
              ),
            ),
          ],
        ),
      );
    }

    final detail = selected.value == null
        ? const _NothingSelected()
        : EntityDetail(entityId: selected.value!);
    final table =
        sizeClass == SizeClass.expanded &&
        (tab.value == DirectoryTab.people ||
            tab.value == DirectoryTab.companies);
    final colors = context.strataColors;
    if (!table) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            width: StrataLayout.listPaneWidth,
            child: ColoredBox(
              color: colors.background,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _PaneHeader(tab: tab.value),
                  tabs,
                  Expanded(
                    child: CustomScrollView(
                      slivers: [
                        SliverPadding(
                          padding: const EdgeInsets.fromLTRB(
                            StrataSpacing.s3,
                            StrataSpacing.s2,
                            StrataSpacing.s3,
                            0,
                          ),
                          sliver: SliverList.list(
                            children: [
                              search,
                              filters,
                              const SuggestionStrip(),
                            ],
                          ),
                        ),
                        SliverFillRemaining(hasScrollBody: false, child: list),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
          VerticalDivider(width: 1, color: colors.border),
          Expanded(child: detail),
        ],
      );
    }

    return ColoredBox(
      color: colors.surface,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(
                    StrataSpacing.s6,
                    StrataSpacing.s4,
                    StrataSpacing.s6,
                    StrataSpacing.s2,
                  ),
                  child: Wrap(
                    spacing: StrataSpacing.s4,
                    runSpacing: StrataSpacing.s2,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    alignment: WrapAlignment.spaceBetween,
                    children: [
                      Semantics(
                        header: true,
                        child: Text(
                          context.dirL10n.directoryTitle,
                          style: context.strataText.title,
                        ),
                      ),
                      tabs,
                      NewEntityButton(tab: tab.value),
                    ],
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s6,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      const SuggestionStrip(columns: 2),
                      Wrap(
                        spacing: StrataSpacing.s3,
                        runSpacing: StrataSpacing.s2,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          SizedBox(width: 300, child: search),
                          filters,
                          if (view != null)
                            Text(
                              _count(context, tab.value, view.items.length),
                              style: context.strataText.caption.copyWith(
                                color: colors.text2,
                              ),
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: StrataSpacing.s2),
                Divider(height: 1, color: colors.border),
                Expanded(
                  child: view == null
                      ? list
                      : _EntityTable(
                          view: view,
                          selected: selected.value,
                          onSelect: (id) => selected.value = id,
                        ),
                ),
              ],
            ),
          ),
          if (selected.value != null) ...[
            VerticalDivider(width: 1, color: colors.border),
            SizedBox(
              width: StrataLayout.contextPanelWidth,
              child: _Preview(
                entityId: selected.value!,
                onClose: () => selected.value = null,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

String _count(BuildContext context, DirectoryTab tab, int count) {
  final l10n = context.dirL10n;
  return switch (tab) {
    DirectoryTab.people => l10n.peopleCount(count: count),
    DirectoryTab.companies => l10n.companiesCount(count: count),
    DirectoryTab.documents => l10n.documentsCount(count: count),
    DirectoryTab.places => l10n.placesCount(count: count),
  };
}

String _tabLabel(DirectoryLocalizations l10n, DirectoryTab tab) =>
    switch (tab) {
      DirectoryTab.people => l10n.tabPeople,
      DirectoryTab.companies => l10n.tabCompanies,
      DirectoryTab.documents => l10n.tabDocuments,
      DirectoryTab.places => l10n.tabPlaces,
    };

int _tabCount(DirectoryCounts counts, DirectoryTab tab) => switch (tab) {
  DirectoryTab.people => counts.people,
  DirectoryTab.companies => counts.companies,
  DirectoryTab.documents => counts.documents,
  DirectoryTab.places => counts.places,
};

class _Tabs extends StatelessWidget {
  const new({
    required this.tab,
    required this.counts,
    required this.onSelected,
    required this.dense,
  });

  final DirectoryTab tab;
  final DirectoryCounts? counts;
  final ValueChanged<DirectoryTab> onSelected;
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final all = counts;
    return Semantics(
      label: l10n.tabsLabel,
      container: true,
      explicitChildNodes: true,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        padding: EdgeInsets.symmetric(
          horizontal: dense ? StrataSpacing.s3 : StrataSpacing.s2,
        ),
        child: Row(
          children: [
            for (final value in DirectoryTab.values)
              Semantics(
                selected: value == tab,
                button: true,
                child: InkWell(
                  onTap: () => onSelected(value),
                  child: Container(
                    constraints: const BoxConstraints(minHeight: 48),
                    padding: const EdgeInsets.symmetric(
                      horizontal: StrataSpacing.s3,
                    ),
                    decoration: BoxDecoration(
                      border: Border(
                        bottom: BorderSide(
                          color: value == tab
                              ? colors.accent
                              : Colors.transparent,
                          width: 2,
                        ),
                      ),
                    ),
                    alignment: Alignment.center,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          _tabLabel(l10n, value),
                          style: text.bodySmall
                              .withWeight(
                                value == tab
                                    ? FontWeight.w700
                                    : FontWeight.w500,
                              )
                              .copyWith(
                                color: value == tab
                                    ? colors.accentText
                                    : colors.text2,
                              ),
                        ),
                        if (all != null) ...[
                          const SizedBox(width: StrataSpacing.s1),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 6),
                            decoration: BoxDecoration(
                              color: value == tab
                                  ? colors.accentTint
                                  : colors.surface2,
                              borderRadius: StrataRadii.pillRadius,
                            ),
                            child: Text(
                              '${_tabCount(all, value)}',
                              style: text.caption.copyWith(
                                color: value == tab
                                    ? colors.accentText
                                    : colors.text2,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _SearchField extends StatelessWidget {
  const new({required this.tab, required this.onChanged});

  final DirectoryTab tab;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final label = switch (tab) {
      DirectoryTab.people => l10n.searchPeople,
      DirectoryTab.companies => l10n.searchCompanies,
      DirectoryTab.documents => l10n.searchDocuments,
      DirectoryTab.places => l10n.searchPlaces,
    };
    return Semantics(
      label: label,
      textField: true,
      child: TextField(
        key: ValueKey(tab),
        onChanged: onChanged,
        textInputAction: TextInputAction.search,
        decoration: InputDecoration(
          isDense: true,
          hintText: tab == DirectoryTab.documents
              ? l10n.searchHintDocuments
              : l10n.searchHint,
          prefixIcon: const Icon(Icons.search, size: 20),
          fillColor: context.strataColors.surface,
        ),
      ),
    );
  }
}

class _Filters extends StatelessWidget {
  const new({required this.tab});

  final DirectoryTab tab;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final names = switch (tab) {
      DirectoryTab.people => [
        l10n.filterTag,
        l10n.filterRole,
        l10n.filterCompany,
      ],
      DirectoryTab.companies => [l10n.filterTag, l10n.filterIndustry],
      DirectoryTab.documents => [
        l10n.filterType,
        l10n.filterStatus,
        l10n.filterPlace,
        l10n.filterHolder,
        l10n.filterExpiring,
      ],
      DirectoryTab.places => [l10n.filterTag],
    };
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
      child: Semantics(
        label: l10n.filtersLabel,
        container: true,
        explicitChildNodes: true,
        child: Tooltip(
          message: l10n.filtersUnavailable,
          child: Wrap(
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s1,
            children: [
              for (final name in names)
                InputChip(
                  label: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(name),
                      Icon(Icons.expand_more, size: 16, color: colors.text2),
                    ],
                  ),
                  isEnabled: false,
                  labelStyle: context.strataText.bodySmall.copyWith(
                    color: colors.text2,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _RowList extends StatelessWidget {
  const new({required this.view, required this.selected, required this.onTap});

  final DirectoryView view;
  final String? selected;
  final ValueChanged<String> onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    if (view.items.isEmpty) {
      final empty = switch (view.tab) {
        DirectoryTab.people => l10n.emptyPeople,
        DirectoryTab.companies => l10n.emptyCompanies,
        DirectoryTab.documents => l10n.emptyDocuments,
        DirectoryTab.places => l10n.emptyPlaces,
      };
      return StrataEmptyState(
        icon: DirectoryScreen.icon,
        title: view.query.isEmpty ? empty : l10n.noMatches(query: view.query),
        message: view.query.isEmpty ? l10n.emptyMessage : l10n.noMatchesMessage,
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        StrataSpacing.s3,
        StrataSpacing.s2,
        StrataSpacing.s3,
        StrataSpacing.s6,
      ),
      child: Card(
        clipBehavior: Clip.antiAlias,
        child: Column(
          children: [
            for (var i = 0; i < view.items.length; i++) ...[
              if (i > 0) Divider(height: 1, color: colors.border),
              DirectoryRow(
                item: view.items[i],
                kind: tabKind(view.tab),
                selected: view.items[i].id == selected,
                onTap: () => onTap(view.items[i].id),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// A directory row: kind avatar, name, first alias, and the core's subtitle
/// (role · company, document status and location, parent place).
class DirectoryRow extends StatelessWidget {
  /// Creates the row.
  const new({
    required this.item,
    required this.kind,
    required this.onTap,
    super.key,
    this.selected = false,
  });

  /// The item.
  final DirectoryItem item;

  /// Its kind.
  final NodeKind kind;

  /// Opens or selects it.
  final VoidCallback onTap;

  /// Selected in the list pane.
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final subtitle = item.subtitle;
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? colors.accentTint : colors.surface,
        child: InkWell(
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s3,
              vertical: StrataSpacing.s3,
            ),
            child: Row(
              children: [
                EntityAvatar(kind: kind),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Wrap(
                        spacing: StrataSpacing.s2,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          Text(
                            item.title,
                            style: text.body.withWeight(FontWeight.w600),
                          ),
                          for (final alias in item.aliases.take(1))
                            Text(
                              alias,
                              style: text.bodySmall.copyWith(
                                color: colors.text2,
                              ),
                            ),
                        ],
                      ),
                      if (subtitle != null)
                        Text(
                          subtitle,
                          style: text.bodySmall.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _PaneHeader extends StatelessWidget {
  const new({required this.tab});

  final DirectoryTab tab;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsetsDirectional.fromSTEB(
      StrataSpacing.s4,
      StrataSpacing.s3,
      StrataSpacing.s2,
      0,
    ),
    child: Row(
      children: [
        Expanded(
          child: Semantics(
            header: true,
            child: Text(
              context.dirL10n.directoryTitle,
              style: context.strataText.title,
            ),
          ),
        ),
        NewEntityButton(tab: tab, iconOnly: true),
      ],
    ),
  );
}

class _NothingSelected extends StatelessWidget {
  const new();

  @override
  Widget build(BuildContext context) => ColoredBox(
    color: context.strataColors.surface,
    child: StrataEmptyState(
      icon: DirectoryScreen.icon,
      title: context.dirL10n.selectSomething,
    ),
  );
}

class _EntityTable extends HookWidget {
  const new({
    required this.view,
    required this.selected,
    required this.onSelect,
  });

  final DirectoryView view;
  final String? selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = EntityLinks.of(context).onOpenEntity;
    final items = view.items;
    if (items.isEmpty) {
      return _RowList(view: view, selected: null, onTap: onSelect);
    }
    final index = items.indexWhere((item) => item.id == selected);
    void move(int delta) {
      final next = (index + delta).clamp(0, items.length - 1);
      onSelect(items[next].id);
    }

    final head = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2);
    Widget cells(List<Widget> children) => Row(
      children: [
        Expanded(flex: 30, child: children[0]),
        Expanded(flex: 30, child: children[1]),
        Expanded(flex: 40, child: children[2]),
      ],
    );
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.arrowDown): () => move(1),
        const SingleActivator(LogicalKeyboardKey.arrowUp): () => move(-1),
        const SingleActivator(LogicalKeyboardKey.enter): () {
          if (index >= 0) open?.call(items[index].id);
        },
      },
      child: Focus(
        autofocus: true,
        child: Semantics(
          label: l10n.tableCaption(tab: _tabLabel(l10n, view.tab)),
          container: true,
          explicitChildNodes: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ColoredBox(
                color: colors.background,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s6,
                    vertical: StrataSpacing.s2,
                  ),
                  child: cells([
                    Text(l10n.colName, style: head),
                    Text(l10n.colAliases, style: head),
                    Text(l10n.colDetails, style: head),
                  ]),
                ),
              ),
              Expanded(
                child: ListView.separated(
                  itemCount: items.length,
                  separatorBuilder: (_, _) =>
                      Divider(height: 1, color: colors.border),
                  itemBuilder: (context, i) {
                    final item = items[i];
                    final isSelected = item.id == selected;
                    return Semantics(
                      selected: isSelected,
                      button: true,
                      child: Material(
                        color: isSelected ? colors.accentTint : colors.surface,
                        child: InkWell(
                          onTap: () => onSelect(item.id),
                          onDoubleTap: open == null
                              ? null
                              : () => open(item.id),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: StrataSpacing.s6,
                              vertical: StrataSpacing.s2,
                            ),
                            child: cells([
                              Row(
                                children: [
                                  EntityAvatar(
                                    kind: tabKind(view.tab),
                                    size: 28,
                                  ),
                                  const SizedBox(width: StrataSpacing.s2),
                                  Flexible(
                                    child: Text(
                                      item.title,
                                      style: text.bodySmall.withWeight(
                                        FontWeight.w600,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                              Wrap(
                                spacing: StrataSpacing.s2,
                                children: [
                                  for (final alias in item.aliases)
                                    Text(alias, style: text.bodySmall),
                                ],
                              ),
                              Text(
                                item.subtitle ?? '',
                                style: text.bodySmall.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            ]),
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ),
              Divider(height: 1, color: colors.border),
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: StrataSpacing.s6,
                  vertical: StrataSpacing.s2,
                ),
                child: Row(
                  children: [
                    const KeyboardHintChip(keys: ['↑', '↓']),
                    const SizedBox(width: StrataSpacing.s1),
                    const KeyboardHintChip(keys: ['↵']),
                    const SizedBox(width: StrataSpacing.s2),
                    Flexible(
                      child: Text(
                        l10n.keyboardHint,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Preview extends ConsumerWidget {
  const new({required this.entityId, required this.onClose});

  final String entityId;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = EntityLinks.of(context).onOpenEntity;
    final entity = ref.watch(entityProvider(entityId)).value?.entity;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.previewOf(title: entity?.title ?? ''),
      child: ColoredBox(
        color: colors.background,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsetsDirectional.fromSTEB(
                StrataSpacing.s4,
                StrataSpacing.s2,
                StrataSpacing.s1,
                0,
              ),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.preview,
                      style: text.caption
                          .withWeight(FontWeight.w700)
                          .copyWith(color: colors.text2),
                    ),
                  ),
                  IconButton(
                    tooltip: l10n.closePreview,
                    onPressed: onClose,
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
            Expanded(
              child: entity == null
                  ? const SizedBox()
                  : SingleChildScrollView(
                      padding: const EdgeInsets.symmetric(
                        horizontal: StrataSpacing.s4,
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Row(
                            children: [
                              EntityAvatar(
                                kind: entityKindOf(entity.kind),
                                size: 52,
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
                                        style: text.titleSmall,
                                      ),
                                    ),
                                    for (final alias in entity.aliases.take(1))
                                      Text(
                                        alias,
                                        style: text.bodySmall.copyWith(
                                          color: colors.text2,
                                        ),
                                      ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                          PageSection(
                            title: l10n.summary,
                            caption: l10n.aiMaintained,
                            children: [
                              Text(
                                entity.summary ?? l10n.noSummary,
                                style: text.bodySmall,
                              ),
                            ],
                          ),
                          PageSection(
                            title: l10n.openItems,
                            children: [
                              CitedBullets(entity.openItems, openItem: true),
                            ],
                          ),
                          if (entity.aliases.isNotEmpty)
                            PageSection(
                              title: l10n.colAliases,
                              children: [
                                Wrap(
                                  spacing: StrataSpacing.s2,
                                  runSpacing: StrataSpacing.s1,
                                  children: [
                                    for (final alias in entity.aliases)
                                      Chip(label: Text(alias)),
                                  ],
                                ),
                              ],
                            ),
                        ],
                      ),
                    ),
            ),
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Row(
                children: [
                  if (open != null)
                    Expanded(
                      child: FilledButton(
                        onPressed: () => open(entityId),
                        child: Text(l10n.openPage),
                      ),
                    ),
                  const SizedBox(width: StrataSpacing.s2),
                  Expanded(
                    child: Tooltip(
                      message: l10n.mergeUnavailable,
                      child: OutlinedButton(
                        onPressed: null,
                        style: OutlinedButton.styleFrom(
                          disabledForegroundColor: colors.text2,
                        ),
                        child: Text(l10n.merge),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
