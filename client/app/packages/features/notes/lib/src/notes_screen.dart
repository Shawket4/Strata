import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_notes/src/context/note_context.dart';
import 'package:strata_notes/src/detail/note_detail.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_notes/src/list/notes_list_pane.dart';
import 'package:strata_notes/src/notes_scope.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the notes feature (PLAN §11 screens 4 and the Notes
/// destination): the notes list and the note view, adaptive per size class.
///
/// * compact: the list, or — with a selected note — the full-screen note page
///   (Note / Links / History tabs, formatting toolbar);
/// * medium: list + note, context (backlinks, graph, history) as a drawer;
/// * expanded: list + note + context panel.
///
/// [folder] and [selectedNoteId] come from the route. When the host passes
/// no [onOpenFolder] / [onOpenNote] / [onCloseNote], the screen keeps the
/// folder and selection itself (ephemeral UI state).
class NotesScreen extends StatefulWidget {
  /// Creates the notes screen.
  const new({
    super.key,
    this.folder = '',
    this.selectedNoteId,
    this.onOpenFolder,
    this.onOpenNote,
    this.onCloseNote,
    this.onOpenLink,
    this.onOpenConflict,
    this.onOpenLocalMap,
  });

  /// The folder shown (`''` = vault root).
  final String folder;

  /// The note shown next to (or, on compact, instead of) the list.
  final String? selectedNoteId;

  /// Navigates to a folder.
  final ValueChanged<String>? onOpenFolder;

  /// Navigates to a note.
  final ValueChanged<String>? onOpenNote;

  /// Leaves the note (compact back).
  final VoidCallback? onCloseNote;

  /// Opens a wikilink (its source text, `[[Note|alias]]`).
  final ValueChanged<String>? onOpenLink;

  /// Opens the conflict screen for an op ID (sync feature).
  final ValueChanged<String>? onOpenConflict;

  /// Opens the local mind map of a note (maps feature).
  final ValueChanged<String>? onOpenLocalMap;

  /// The icon that represents this feature.
  static const IconData icon = Icons.description_outlined;

  @override
  State<NotesScreen> createState() => _NotesScreenState();
}

class _NotesScreenState extends State<NotesScreen> {
  late String _folder = widget.folder;
  late String? _selected = widget.selectedNoteId;
  bool _drawerOpen = false;
  bool _panelVisible = true;
  final FocusNode _searchFocus = FocusNode(debugLabel: 'notes search');

  @override
  void didUpdateWidget(NotesScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.folder != oldWidget.folder) _folder = widget.folder;
    if (widget.selectedNoteId != oldWidget.selectedNoteId) {
      _selected = widget.selectedNoteId;
    }
  }

  @override
  void dispose() {
    _searchFocus.dispose();
    super.dispose();
  }

  void _openFolder(String folder) {
    final open = widget.onOpenFolder;
    if (open != null) {
      open(folder);
    } else {
      setState(() => _folder = folder);
    }
  }

  void _openNote(String id) {
    final open = widget.onOpenNote;
    if (open != null) {
      open(id);
    } else {
      setState(() => _selected = id);
    }
  }

  void _closeNote() {
    final close = widget.onCloseNote;
    if (close != null) {
      close();
    } else {
      setState(() => _selected = null);
    }
  }

  void _toggleContext(SizeClass sizeClass) => setState(() {
    if (sizeClass == SizeClass.medium) {
      _drawerOpen = !_drawerOpen;
    } else {
      _panelVisible = !_panelVisible;
    }
  });

  @override
  Widget build(BuildContext context) {
    final sizeClass = SizeClass.of(context);
    final selected = _selected;
    final actions = NoteActions(
      onOpenNote: _openNote,
      onOpenLink: widget.onOpenLink,
      onOpenConflict: widget.onOpenConflict,
      onOpenLocalMap: widget.onOpenLocalMap,
    );
    final list = NotesListPane(
      folder: _folder,
      selectedNoteId: selected,
      onOpenFolder: _openFolder,
      onOpenNote: _openNote,
      searchFocusNode: _searchFocus,
    );
    return NotesLocalizationsScope(
      child: Builder(
        builder: (context) {
          final l10n = NotesLocalizations.of(context);
          if (sizeClass == SizeClass.compact) {
            return selected == null
                ? list
                : CompactNotePage(
                    key: ValueKey(selected),
                    noteId: selected,
                    onBack: _closeNote,
                    actions: actions,
                  );
          }
          final contextVisible = sizeClass == SizeClass.medium
              ? _drawerOpen
              : _panelVisible;
          final detail = selected == null
              ? Material(
                  color: context.strataColors.surface,
                  child: Center(
                    child: SingleChildScrollView(
                      child: StrataEmptyState(
                        icon: NotesScreen.icon,
                        title: l10n.selectNoteTitle,
                        message: l10n.selectNoteMessage,
                      ),
                    ),
                  ),
                )
              : NoteDetailPane(
                  key: ValueKey(selected),
                  noteId: selected,
                  contextVisible: contextVisible,
                  onToggleContext: () => _toggleContext(sizeClass),
                  actions: actions,
                );
          final panel = selected == null || !contextVisible
              ? null
              : _ContextPanelFor(
                  noteId: selected,
                  actions: actions,
                  onClose: sizeClass == SizeClass.medium
                      ? () => setState(() => _drawerOpen = false)
                      : null,
                );
          return CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.keyF, meta: true):
                  _searchFocus.requestFocus,
              const SingleActivator(LogicalKeyboardKey.keyF, control: true):
                  _searchFocus.requestFocus,
              const SingleActivator(
                LogicalKeyboardKey.period,
                meta: true,
              ): () =>
                  _toggleContext(sizeClass),
              const SingleActivator(
                LogicalKeyboardKey.period,
                control: true,
              ): () =>
                  _toggleContext(sizeClass),
            },
            child: Focus(
              autofocus: true,
              child: StrataPanes(
                list: list,
                detail: detail,
                contextPanel: panel,
                contextPanelOpen: panel != null,
                contextPanelLabel: l10n.contextLabel,
                dismissLabel: l10n.hideContext,
                onContextPanelClosed: () => setState(() => _drawerOpen = false),
              ),
            ),
          );
        },
      ),
    );
  }
}

class _ContextPanelFor extends ConsumerWidget {
  const new({required this.noteId, required this.actions, this.onClose});

  final String noteId;
  final NoteActions actions;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final note = ref.watch(noteProvider(noteId)).value?.note;
    if (note == null) return const SizedBox.shrink();
    final openMap = actions.onOpenLocalMap;
    return NoteContextPanel(
      note: note,
      onOpenNote: actions.onOpenNote,
      onOpenMap: openMap == null ? null : () => openMap(noteId),
      onClose: onClose,
    );
  }
}
