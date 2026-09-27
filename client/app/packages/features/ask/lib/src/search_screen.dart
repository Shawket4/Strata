import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_ask/src/common.dart';
import 'package:strata_ask/src/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Search (PLAN §11 screen 9): keyword (local, offline), semantic and hybrid
/// (server) modes — whether a mode can run comes from the core — and
/// results with snippets. Expanded adds a preview of the selected result.
class SearchScreen extends StatelessWidget {
  /// Creates Search.
  const new({super.key, this.initialQuery = '', this.onOpenNote});

  /// Query shown first.
  final String initialQuery;

  /// Opens a note.
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context) => AskLocalizationScope(
    child: _Search(initialQuery: initialQuery, onOpenNote: onOpenNote),
  );
}

String _modeLabel(AskLocalizations l10n, SearchMode mode) => switch (mode) {
  SearchMode.keyword => l10n.modeKeyword,
  SearchMode.semantic => l10n.modeSemantic,
  SearchMode.hybrid => l10n.modeHybrid,
};

NodeKind _kind(String kind) => switch (kind) {
  'concept' => NodeKind.concept,
  'person' => NodeKind.person,
  'company' => NodeKind.company,
  'document' => NodeKind.document,
  'place' => NodeKind.place,
  _ => NodeKind.note,
};

class _Search extends HookConsumerWidget {
  const new({required this.initialQuery, required this.onOpenNote});

  final String initialQuery;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final query = useState(initialQuery);
    final mode = useState(SearchMode.keyword);
    final selected = useState<String?>(null);
    final controller = useTextEditingController(text: initialQuery);
    final expanded = SizeClass.of(context) == SizeClass.expanded;
    final async = query.value.isEmpty
        ? null
        : ref.watch(searchProvider(query.value, mode.value));
    final open = onOpenNote;

    Widget results;
    if (async == null) {
      results = StrataEmptyState(
        icon: Icons.search,
        title: l10n.searchPrompt,
        message: l10n.searchPromptMessage,
      );
    } else {
      results = switch (async) {
        AsyncData(:final value) => _Results(
          view: value,
          selected: selected.value,
          onTap: (hit) {
            if (expanded) {
              selected.value = hit.noteId;
            } else {
              open?.call(hit.noteId, null);
            }
          },
        ),
        AsyncError(:final error) => AskError(error: error),
        _ => const AskLoading(),
      };
    }

    final column = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            StrataSpacing.s4,
            StrataSpacing.s3,
            StrataSpacing.s4,
            StrataSpacing.s2,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Semantics(
                header: true,
                child: Text(l10n.searchTitle, style: text.titleSmall),
              ),
              const SizedBox(height: StrataSpacing.s2),
              Semantics(
                label: l10n.searchField,
                textField: true,
                child: TextField(
                  controller: controller,
                  autofocus: true,
                  textInputAction: TextInputAction.search,
                  onChanged: (value) {
                    query.value = value;
                    selected.value = null;
                  },
                  decoration: InputDecoration(
                    hintText: l10n.searchHint,
                    prefixIcon: const Icon(Icons.search),
                    fillColor: colors.surface,
                  ),
                ),
              ),
              const SizedBox(height: StrataSpacing.s2),
              Semantics(
                label: l10n.searchMode,
                container: true,
                explicitChildNodes: true,
                child: SegmentedButton<SearchMode>(
                  showSelectedIcon: false,
                  segments: [
                    for (final value in SearchMode.values)
                      ButtonSegment(
                        value: value,
                        label: Text(_modeLabel(l10n, value)),
                      ),
                  ],
                  selected: {mode.value},
                  onSelectionChanged: (values) {
                    mode.value = values.first;
                    selected.value = null;
                  },
                ),
              ),
            ],
          ),
        ),
        Expanded(child: results),
      ],
    );

    if (!expanded) {
      return ColoredBox(color: colors.background, child: column);
    }
    final id = selected.value;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          width: 480,
          child: ColoredBox(color: colors.background, child: column),
        ),
        VerticalDivider(width: 1, color: colors.border),
        Expanded(
          child: ColoredBox(
            color: colors.surface,
            child: id == null
                ? StrataEmptyState(
                    icon: Icons.article_outlined,
                    title: l10n.previewHint,
                  )
                : _Preview(noteId: id, onOpenNote: open),
          ),
        ),
      ],
    );
  }
}

class _Results extends StatelessWidget {
  const new({required this.view, required this.selected, required this.onTap});

  final SearchView view;
  final String? selected;
  final ValueChanged<SearchHit> onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final mode = _modeLabel(l10n, view.mode);
    final notice = switch (view.availability) {
      Availability.available => null as String?,
      Availability.offline => l10n.modeOffline(mode: mode),
      Availability.notYetAvailable => l10n.modeNotYet(mode: mode),
      Availability.notAllowed => l10n.modeNotAllowed(mode: mode),
    };
    return ListView(
      padding: const EdgeInsets.fromLTRB(
        StrataSpacing.s4,
        0,
        StrataSpacing.s4,
        StrataSpacing.s6,
      ),
      children: [
        if (notice != null)
          Semantics(
            liveRegion: true,
            container: true,
            child: Container(
              margin: const EdgeInsets.only(bottom: StrataSpacing.s3),
              padding: const EdgeInsets.all(StrataSpacing.s3),
              decoration: BoxDecoration(
                color: colors.warningTint,
                borderRadius: StrataRadii.cardRadius,
              ),
              child: Row(
                children: [
                  Icon(
                    Icons.cloud_off_outlined,
                    size: 18,
                    color: colors.warningText,
                  ),
                  const SizedBox(width: StrataSpacing.s2),
                  Expanded(
                    child: Text(
                      notice,
                      style: text.bodySmall.copyWith(color: colors.warningText),
                    ),
                  ),
                ],
              ),
            ),
          ),
        if (view.results.isEmpty && notice == null)
          StrataEmptyState(
            icon: Icons.search_off,
            title: l10n.noResults(query: view.query),
            message: l10n.noResultsMessage,
          )
        else if (view.results.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
            child: Text(
              l10n.resultsCount(count: view.results.length),
              style: text.caption.copyWith(color: colors.text2),
            ),
          ),
          Card(
            clipBehavior: Clip.antiAlias,
            child: Column(
              children: [
                for (var i = 0; i < view.results.length; i++) ...[
                  if (i > 0) Divider(height: 1, color: colors.border),
                  _HitTile(
                    hit: view.results[i],
                    selected: view.results[i].noteId == selected,
                    onTap: () => onTap(view.results[i]),
                  ),
                ],
              ],
            ),
          ),
        ],
      ],
    );
  }
}

class _HitTile extends StatelessWidget {
  const new({required this.hit, required this.selected, required this.onTap});

  final SearchHit hit;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? colors.accentTint : colors.surface,
        child: InkWell(
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.all(StrataSpacing.s3),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(top: 3),
                  child: NodeKindGlyph(kind: _kind(hit.kind), size: 18),
                ),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        hit.title,
                        style: text.body.withWeight(FontWeight.w600),
                      ),
                      Text(
                        hit.path,
                        textDirection: TextDirection.ltr,
                        style: text.monoSmall.copyWith(color: colors.text2),
                      ),
                      if (hit.snippet.isNotEmpty)
                        Text(
                          hit.snippet,
                          textAlign: TextAlign.start,
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

class _Preview extends ConsumerWidget {
  const new({required this.noteId, required this.onOpenNote});

  final String noteId;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final note = ref.watch(noteProvider(noteId)).value?.note;
    final open = onOpenNote;
    if (note == null) return const AskLoading();
    return Semantics(
      label: l10n.preview,
      container: true,
      explicitChildNodes: true,
      child: ListView(
        padding: const EdgeInsets.all(StrataSpacing.s6),
        children: [
          Semantics(header: true, child: Text(note.title, style: text.title)),
          Text(
            note.path,
            textDirection: TextDirection.ltr,
            style: text.monoSmall.copyWith(color: colors.text2),
          ),
          if (note.tags.isNotEmpty) ...[
            const SizedBox(height: StrataSpacing.s2),
            Wrap(
              spacing: StrataSpacing.s1,
              runSpacing: StrataSpacing.s1,
              children: [for (final tag in note.tags) Chip(label: Text(tag))],
            ),
          ],
          const SizedBox(height: StrataSpacing.s4),
          Text(
            note.content,
            maxLines: 24,
            overflow: TextOverflow.fade,
            textAlign: TextAlign.start,
            style: text.bodySmall,
          ),
          const SizedBox(height: StrataSpacing.s4),
          if (open != null)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: FilledButton.icon(
                onPressed: () => open(note.id, null),
                icon: const Icon(Icons.open_in_new, size: 18),
                label: Text(l10n.openNote),
              ),
            ),
        ],
      ),
    );
  }
}
