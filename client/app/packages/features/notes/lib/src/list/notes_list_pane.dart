import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The notes list: the core's folder breadcrumb and note count, search
/// field, subfolders, and notes with snippets (each in its own direction)
/// and edit labels, the selected note highlighted. With a query the list
/// shows the core's keyword search limited to the folder
/// (`search_in_folder`), with the matched terms marked.
class NotesListPane extends ConsumerStatefulWidget {
  /// Creates the list of [folder].
  const new({
    required this.folder,
    required this.onOpenFolder,
    required this.onOpenNote,
    super.key,
    this.selectedNoteId,
    this.searchFocusNode,
  });

  /// The folder shown (`''` = vault root).
  final String folder;

  /// The selected note.
  final String? selectedNoteId;

  /// Opens a folder.
  final ValueChanged<String> onOpenFolder;

  /// Opens a note.
  final ValueChanged<String> onOpenNote;

  /// Focus of the search field (⌘/Ctrl-F).
  final FocusNode? searchFocusNode;

  @override
  ConsumerState<NotesListPane> createState() => _NotesListPaneState();
}

class _NotesListPaneState extends ConsumerState<NotesListPane> {
  final TextEditingController _query = TextEditingController();

  @override
  void initState() {
    super.initState();
    _query.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _query.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final query = _query.text;
    final list = ref.watch(notesListProvider(widget.folder)).value;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.notesInFolderLabel(
        folder: widget.folder.isEmpty ? l10n.notesRoot : widget.folder,
      ),
      child: Material(
        color: colors.surface,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _Header(
              breadcrumb: list?.breadcrumb ?? const [],
              noteCount: list?.noteCount,
              query: _query,
              focusNode: widget.searchFocusNode,
              onOpenFolder: widget.onOpenFolder,
            ),
            Expanded(
              child: query.isEmpty
                  ? _FolderList(
                      folder: widget.folder,
                      selectedNoteId: widget.selectedNoteId,
                      onOpenFolder: widget.onOpenFolder,
                      onOpenNote: widget.onOpenNote,
                    )
                  : _SearchResults(
                      query: query,
                      folder: widget.folder,
                      selectedNoteId: widget.selectedNoteId,
                      onOpenNote: widget.onOpenNote,
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const new({
    required this.breadcrumb,
    required this.noteCount,
    required this.query,
    required this.onOpenFolder,
    this.focusNode,
  });

  final List<FolderItem> breadcrumb;
  final int? noteCount;
  final TextEditingController query;
  final FocusNode? focusNode;
  final ValueChanged<String> onOpenFolder;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final count = noteCount;
    final chevron = Icon(
      Icons.chevron_right,
      size: 18,
      color: colors.text2,
      textDirection: Directionality.of(context),
    );
    Widget crumb(String label, String path) => TextButton(
      onPressed: () => onOpenFolder(path),
      style: TextButton.styleFrom(
        foregroundColor: colors.text2,
        minimumSize: const Size(48, StrataLayout.minTouchTarget),
        padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s1),
      ),
      child: Text(label),
    );
    Widget current(String label) => Semantics(
      header: true,
      child: Text(label, style: text.titleSmall.copyWith(color: colors.text)),
    );
    return Container(
      padding: const EdgeInsets.fromLTRB(
        StrataSpacing.s3,
        StrataSpacing.s3,
        StrataSpacing.s3,
        StrataSpacing.s3,
      ),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: colors.border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Semantics(
            container: true,
            explicitChildNodes: true,
            label: l10n.folderBreadcrumbLabel,
            child: Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if (breadcrumb.isEmpty)
                  Padding(
                    padding: const EdgeInsetsDirectional.only(
                      start: StrataSpacing.s1,
                    ),
                    child: current(l10n.notesRoot),
                  )
                else ...[
                  crumb(l10n.notesRoot, ''),
                  for (final (i, folder) in breadcrumb.indexed) ...[
                    chevron,
                    if (i == breadcrumb.length - 1)
                      current(folder.name)
                    else
                      crumb(folder.name, folder.path),
                  ],
                ],
                if (count != null)
                  Padding(
                    padding: const EdgeInsetsDirectional.only(
                      start: StrataSpacing.s2,
                    ),
                    child: Text(
                      l10n.folderNoteCount(count: count),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: StrataSpacing.s2),
          TextField(
            controller: query,
            focusNode: focusNode,
            textInputAction: TextInputAction.search,
            decoration: InputDecoration(
              isDense: true,
              hintText: l10n.searchNotesLabel,
              prefixIcon: const Icon(Icons.search, size: 20),
              suffixIcon: query.text.isEmpty
                  ? null
                  : IconButton(
                      tooltip: l10n.clearSearch,
                      icon: const Icon(Icons.close, size: 18),
                      onPressed: query.clear,
                    ),
            ),
          ),
        ],
      ),
    );
  }
}

class _FolderList extends ConsumerWidget {
  const new({
    required this.folder,
    required this.selectedNoteId,
    required this.onOpenFolder,
    required this.onOpenNote,
  });

  final String folder;
  final String? selectedNoteId;
  final ValueChanged<String> onOpenFolder;
  final ValueChanged<String> onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final list = ref.watch(notesListProvider(folder));
    return switch (list) {
      AsyncData(:final value)
          when value.folders.isEmpty && value.notes.isEmpty =>
        _Centered(
          child: StrataEmptyState(
            icon: Icons.description_outlined,
            title: l10n.emptyFolderTitle,
            message: l10n.emptyFolderMessage,
          ),
        ),
      AsyncData(:final value) => ListView(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s2,
          vertical: StrataSpacing.s1 + 2,
        ),
        children: [
          for (final folder in value.folders)
            _FolderRow(folder: folder, onTap: () => onOpenFolder(folder.path)),
          for (final note in value.notes)
            NoteRow(
              title: note.title,
              titleDir: note.titleDir,
              snippet: note.snippet,
              snippetDir: note.snippetDir,
              updatedLabel: note.updatedLabel,
              pendingSync: note.pendingSync,
              selected: note.id == selectedNoteId,
              onTap: () => onOpenNote(note.id),
              noteId: note.id,
            ),
        ],
      ),
      AsyncError() => _Centered(
        child: StrataEmptyState(
          icon: Icons.error_outline,
          title: l10n.listErrorTitle,
          message: l10n.listErrorMessage,
          action: StrataAction(
            label: context.l10n.actionRetry,
            icon: Icons.refresh,
            onPressed: () => ref.invalidate(notesListProvider(folder)),
          ),
        ),
      ),
      _ => const Center(child: CircularProgressIndicator()),
    };
  }
}

class _SearchResults extends ConsumerWidget {
  const new({
    required this.query,
    required this.folder,
    required this.selectedNoteId,
    required this.onOpenNote,
  });

  final String query;
  final String folder;
  final String? selectedNoteId;
  final ValueChanged<String> onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final results = ref.watch(
      searchInFolderProvider(
        query,
        SearchMode.keyword,
        folder.isEmpty ? null : folder,
      ),
    );
    return switch (results) {
      AsyncData(:final value)
          when value.availability != Availability.available =>
        _Centered(
          child: StrataEmptyState(
            icon: Icons.search_off,
            title: l10n.searchUnavailable,
          ),
        ),
      AsyncData(:final value) when value.results.isEmpty => _Centered(
        child: StrataEmptyState(
          icon: Icons.search_off,
          title: l10n.noSearchResults(query: query),
        ),
      ),
      AsyncData(:final value) => ListView(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s2,
          vertical: StrataSpacing.s1 + 2,
        ),
        children: [
          for (final hit in value.results)
            NoteRow(
              title: hit.title,
              titleDir: hit.titleDir,
              snippet: hit.snippet,
              snippetDir: hit.snippetDir,
              highlights: hit.highlights,
              pendingSync: false,
              selected: hit.noteId == selectedNoteId,
              onTap: () => onOpenNote(hit.noteId),
              noteId: hit.noteId,
            ),
        ],
      ),
      AsyncError() => _Centered(
        child: StrataEmptyState(
          icon: Icons.search_off,
          title: l10n.searchUnavailable,
        ),
      ),
      _ => const Center(child: CircularProgressIndicator()),
    };
  }
}

class _Centered extends StatelessWidget {
  const new({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Center(child: SingleChildScrollView(child: child));
}

class _FolderRow extends StatelessWidget {
  const new({required this.folder, required this.onTap});

  final FolderItem folder;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final count = l10n.folderNoteCount(count: folder.noteCount);
    return Semantics(
      button: true,
      label: l10n.folderRowSemantics(name: folder.name, count: count),
      excludeSemantics: true,
      onTap: onTap,
      child: InkWell(
        borderRadius: StrataRadii.inputRadius,
        onTap: onTap,
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minHeight: StrataLayout.minTouchTarget,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
            child: Row(
              children: [
                Icon(Icons.folder_outlined, size: 20, color: colors.text2),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Text(
                    folder.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: text.body.copyWith(color: colors.text),
                  ),
                ),
                const SizedBox(width: StrataSpacing.s2),
                Text(count, style: text.caption.copyWith(color: colors.text2)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// A note in the list: title and snippet (the core's plain first line), each
/// in its own direction, the core's edit label, a dot while it has unsynced
/// changes, the search highlights; highlighted when selected. Right-click
/// (or long-press) opens its context menu.
class NoteRow extends ConsumerWidget {
  /// Creates the row.
  const new({
    required this.noteId,
    required this.title,
    required this.snippet,
    required this.pendingSync,
    required this.selected,
    required this.onTap,
    super.key,
    this.titleDir = TextDir.neutral,
    this.snippetDir = TextDir.neutral,
    this.updatedLabel = '',
    this.highlights = const [],
  });

  /// Note ID.
  final String noteId;

  /// Title.
  final String title;

  /// Direction of [title].
  final TextDir titleDir;

  /// Snippet.
  final String snippet;

  /// Direction of [snippet].
  final TextDir snippetDir;

  /// When it was edited ("14:31", "Sat").
  final String updatedLabel;

  /// Matched terms in [snippet].
  final List<HighlightSpan> highlights;

  /// Has unsynced changes.
  final bool pendingSync;

  /// Is the open note.
  final bool selected;

  /// Opens the note.
  final VoidCallback onTap;

  Future<void> _menu(BuildContext context, WidgetRef ref, Offset at) async {
    final l10n = NotesLocalizations.of(context);
    final overlay =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    final choice = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        at & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: [
        PopupMenuItem(value: 'open', child: Text(context.l10n.actionOpen)),
        PopupMenuItem(value: 'delete', child: Text(l10n.deleteNote)),
      ],
    );
    if (!context.mounted) return;
    switch (choice) {
      case 'open':
        onTap();
      case 'delete':
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (dialogContext) => AlertDialog(
            title: Text(l10n.deleteNoteTitle(title: title)),
            content: Text(l10n.deleteNoteMessage),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(dialogContext).pop(false),
                child: Text(context.l10n.actionCancel),
              ),
              FilledButton(
                onPressed: () => Navigator.of(dialogContext).pop(true),
                child: Text(context.l10n.actionDelete),
              ),
            ],
          ),
        );
        if (confirmed ?? false) {
          await ref.read(coreApiProvider).deleteNote(id: noteId);
        }
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Material(
        color: selected ? colors.accentTint : Colors.transparent,
        borderRadius: StrataRadii.inputRadius,
        child: MergeSemantics(
          child: GestureDetector(
            onSecondaryTapUp: (details) =>
                unawaited(_menu(context, ref, details.globalPosition)),
            child: InkWell(
              borderRadius: StrataRadii.inputRadius,
              onTap: onTap,
              onLongPress: () {
                final box = context.findRenderObject()! as RenderBox;
                unawaited(
                  _menu(
                    context,
                    ref,
                    box.localToGlobal(box.size.center(Offset.zero)),
                  ),
                );
              },
              child: Semantics(
                selected: selected,
                child: ConstrainedBox(
                  constraints: const BoxConstraints(
                    minHeight: StrataLayout.minTouchTarget,
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: StrataSpacing.s3,
                      vertical: StrataSpacing.s2 + 2,
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Row(
                          children: [
                            Expanded(
                              child: Text(
                                title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                textDirection: textDirectionOf(titleDir),
                                textAlign: TextAlign.start,
                                style: text.body.copyWith(
                                  color: colors.text,
                                  fontSize: StrataTypeScale.bodyDense,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ),
                            if (updatedLabel.isNotEmpty) ...[
                              const SizedBox(width: StrataSpacing.s2),
                              Text(
                                updatedLabel,
                                style: text.caption.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            ],
                            if (pendingSync) ...[
                              const SizedBox(width: StrataSpacing.s2),
                              Semantics(
                                label: l10n.notSyncedYet,
                                child: Container(
                                  width: 8,
                                  height: 8,
                                  decoration: BoxDecoration(
                                    color: colors.warning,
                                    shape: BoxShape.circle,
                                  ),
                                ),
                              ),
                            ],
                          ],
                        ),
                        if (snippet.isNotEmpty)
                          StrataHighlightedText(
                            snippet,
                            highlights: textRangesOf(highlights),
                            maxLines: 2,
                            textDirection: textDirectionOf(snippetDir),
                            style: text.bodySmall.copyWith(
                              color: colors.text2,
                              fontSize: 13,
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
