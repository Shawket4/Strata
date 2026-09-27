import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_notes/src/context/note_context.dart';
import 'package:strata_notes/src/detail/properties_panel.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// Navigation and intents the note view hands to its host.
@immutable
class NoteActions {
  /// Creates the actions.
  const new({
    this.onOpenNote,
    this.onOpenLink,
    this.onOpenConflict,
    this.onOpenLocalMap,
  });

  /// Opens a note by ID.
  final ValueChanged<String>? onOpenNote;

  /// Opens a wikilink (its source text).
  final ValueChanged<String>? onOpenLink;

  /// Opens the conflict screen for an op ID.
  final ValueChanged<String>? onOpenConflict;

  /// Opens the local mind map of a note ID.
  final ValueChanged<String>? onOpenLocalMap;
}

/// Renders the loading / error / not-found states of the note stream, or
/// [content] for a loaded note.
Widget noteStates(
  BuildContext context,
  String noteId,
  AsyncValue<NoteScreen> screen,
  Widget Function(NoteView note) content,
) {
  return switch (screen) {
    AsyncData(:final value) when value.note != null => content(value.note!),
    AsyncData() => Center(
      child: SingleChildScrollView(
        child: StrataEmptyState(
          icon: Icons.search_off,
          title: EditorLocalizations.of(context).noteNotFoundTitle,
          message: EditorLocalizations.of(context).noteNotFoundMessage,
        ),
      ),
    ),
    AsyncError() => Center(
      child: SingleChildScrollView(child: NoteLoadError(noteId: noteId)),
    ),
    _ => const Center(child: CircularProgressIndicator()),
  };
}

VoidCallback? _conflict(NoteView note, NoteActions actions) {
  final opId = note.sync_.conflictOpId;
  final open = actions.onOpenConflict;
  if (note.sync_.kind != NoteSyncKind.conflict || opId == null) return null;
  return open == null ? null : () => open(opId);
}

/// The note view of medium and expanded layouts: a toolbar row (path,
/// status, Save, local map, context panel toggle), then the title, the
/// conflict banner, the Properties panel and the editor in one scroll.
class NoteDetailPane extends StatelessWidget {
  /// Creates the pane for [noteId].
  const new({
    required this.noteId,
    required this.contextVisible,
    required this.onToggleContext,
    super.key,
    this.actions = const NoteActions(),
  });

  /// The note.
  final String noteId;

  /// Whether the context panel (or drawer) is shown.
  final bool contextVisible;

  /// Shows or hides the context panel.
  final VoidCallback onToggleContext;

  /// Navigation and intents.
  final NoteActions actions;

  @override
  Widget build(BuildContext context) => NoteEditorSession(
    noteId: noteId,
    builder: (context, screen, controller) => Material(
      color: context.strataColors.surface,
      child: noteStates(
        context,
        noteId,
        screen,
        (note) => _NoteDetailBody(
          note: note,
          controller: controller,
          contextVisible: contextVisible,
          onToggleContext: onToggleContext,
          actions: actions,
        ),
      ),
    ),
  );
}

class _NoteDetailBody extends StatelessWidget {
  const new({
    required this.note,
    required this.controller,
    required this.contextVisible,
    required this.onToggleContext,
    required this.actions,
  });

  final NoteView note;
  final NoteEditorController controller;
  final bool contextVisible;
  final VoidCallback onToggleContext;
  final NoteActions actions;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final save = saveNote(context, controller);
    final conflict = note.sync_.kind == NoteSyncKind.conflict;
    final openMap = actions.onOpenLocalMap;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyS, meta: true): save,
        const SingleActivator(LogicalKeyboardKey.keyS, control: true): save,
      },
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            constraints: const BoxConstraints(minHeight: 56),
            padding: const EdgeInsetsDirectional.fromSTEB(
              StrataSpacing.s6,
              StrataSpacing.s1,
              StrataSpacing.s3,
              StrataSpacing.s1,
            ),
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: colors.border)),
            ),
            child: Row(
              children: [
                Expanded(
                  child: Wrap(
                    spacing: StrataSpacing.s4,
                    runSpacing: StrataSpacing.s1,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        note.path,
                        style: text.monoSmall.copyWith(color: colors.text2),
                      ),
                      NoteStatusLabel(
                        status: controller.status,
                        pendingOps: note.sync_.pendingOps,
                      ),
                    ],
                  ),
                ),
                if (controller.isDirty)
                  Padding(
                    padding: const EdgeInsetsDirectional.only(
                      end: StrataSpacing.s2,
                    ),
                    child: FilledButton(
                      onPressed: controller.isSaving ? null : save,
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(
                          64,
                          StrataLayout.minTouchTarget,
                        ),
                      ),
                      child: Text(context.l10n.actionSave),
                    ),
                  ),
                IconButton(
                  tooltip: l10n.openLocalMap,
                  icon: const Icon(Icons.hub_outlined),
                  color: colors.text2,
                  onPressed: openMap == null ? null : () => openMap(note.id),
                ),
                IconButton(
                  tooltip: contextVisible ? l10n.hideContext : l10n.showContext,
                  isSelected: contextVisible,
                  icon: const Icon(Icons.view_sidebar_outlined),
                  selectedIcon: const Icon(Icons.view_sidebar),
                  color: colors.text,
                  onPressed: onToggleContext,
                ),
              ],
            ),
          ),
          Expanded(
            child: CustomScrollView(
              slivers: [
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(
                    StrataSpacing.s8 + 4,
                    StrataSpacing.s6,
                    StrataSpacing.s8 + 4,
                    StrataSpacing.s4,
                  ),
                  sliver: SliverList.list(
                    children: [
                      Semantics(
                        header: true,
                        child: Text(
                          note.title,
                          textAlign: TextAlign.start,
                          style: text.display.copyWith(
                            color: colors.text,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      const SizedBox(height: StrataSpacing.s4),
                      if (conflict) ...[
                        NoteConflictBanner(onResolve: _conflict(note, actions)),
                        const SizedBox(height: StrataSpacing.s4),
                      ],
                      PropertiesPanel(
                        note: note,
                        onOpenNote: actions.onOpenNote,
                        hoverCards: true,
                      ),
                    ],
                  ),
                ),
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(
                    StrataSpacing.s8 + 4,
                    0,
                    StrataSpacing.s8 + 4,
                    StrataSpacing.s8,
                  ),
                  sliver: StrataNoteEditor(
                    controller: controller,
                    onOpenLink: actions.onOpenLink,
                    onSave: save,
                  ),
                ),
              ],
            ),
          ),
          CompletionsPanel(controller: controller),
        ],
      ),
    );
  }
}

/// Which compact tab is shown.
enum CompactNoteTab {
  /// The note (properties + editor).
  note,

  /// Relations, backlinks and the local graph slot.
  links,

  /// History.
  history,
}

/// The note view of the compact layout (NoteCompact): full-screen page with
/// back, Note / Links / History tabs, and the formatting toolbar above the
/// keyboard.
class CompactNotePage extends StatefulWidget {
  /// Creates the page for [noteId].
  const new({
    required this.noteId,
    super.key,
    this.onBack,
    this.actions = const NoteActions(),
  });

  /// The note.
  final String noteId;

  /// Back to the list.
  final VoidCallback? onBack;

  /// Navigation and intents.
  final NoteActions actions;

  @override
  State<CompactNotePage> createState() => _CompactNotePageState();
}

class _CompactNotePageState extends State<CompactNotePage> {
  CompactNoteTab _tab = CompactNoteTab.note;
  final FocusNode _editorFocus = FocusNode(debugLabel: 'note editor');

  @override
  void dispose() {
    _editorFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => NoteEditorSession(
    noteId: widget.noteId,
    builder: (context, screen, controller) => Material(
      color: context.strataColors.background,
      child: noteStates(
        context,
        widget.noteId,
        screen,
        (note) => _compactBody(context, note, controller),
      ),
    ),
  );

  Widget _compactBody(
    BuildContext context,
    NoteView note,
    NoteEditorController controller,
  ) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final save = saveNote(context, controller);
    final actions = widget.actions;
    final openMap = actions.onOpenLocalMap;
    final back = widget.onBack;
    final body = switch (_tab) {
      CompactNoteTab.note => Column(
        children: [
          Expanded(
            child: ColoredBox(
              color: colors.surface,
              child: CustomScrollView(
                slivers: [
                  SliverPadding(
                    padding: const EdgeInsets.fromLTRB(
                      StrataSpacing.s5,
                      StrataSpacing.s4,
                      StrataSpacing.s5,
                      StrataSpacing.s3,
                    ),
                    sliver: SliverList.list(
                      children: [
                        PropertiesPanel(
                          note: note,
                          collapsible: true,
                          onOpenNote: actions.onOpenNote,
                        ),
                        const SizedBox(height: StrataSpacing.s4),
                        Semantics(
                          header: true,
                          child: Text(
                            note.title,
                            style: text.display.copyWith(
                              color: colors.text,
                              fontSize: 26,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        if (note.sync_.kind == NoteSyncKind.conflict) ...[
                          const SizedBox(height: StrataSpacing.s3),
                          NoteConflictBanner(
                            onResolve: _conflict(note, actions),
                          ),
                        ],
                      ],
                    ),
                  ),
                  SliverPadding(
                    padding: const EdgeInsets.fromLTRB(
                      StrataSpacing.s5,
                      0,
                      StrataSpacing.s5,
                      StrataSpacing.s6,
                    ),
                    sliver: StrataNoteEditor(
                      controller: controller,
                      focusNode: _editorFocus,
                      onOpenLink: actions.onOpenLink,
                      onSave: save,
                    ),
                  ),
                ],
              ),
            ),
          ),
          CompletionsPanel(controller: controller),
          FormattingToolbar(
            controller: controller,
            onOpenLink: actions.onOpenLink,
            onHideKeyboard: _editorFocus.unfocus,
          ),
        ],
      ),
      CompactNoteTab.links => ListView(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [
          PropertiesPanel(note: note, onOpenNote: actions.onOpenNote),
          const SizedBox(height: StrataSpacing.s4),
          BacklinksSection(
            groups: note.backlinks,
            onOpenNote: actions.onOpenNote,
          ),
          const SizedBox(height: StrataSpacing.s3),
          LocalGraphSlot(
            onOpenMap: openMap == null ? null : () => openMap(note.id),
          ),
        ],
      ),
      CompactNoteTab.history => ListView(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [HistorySection(availability: note.history)],
      ),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          constraints: const BoxConstraints(minHeight: 56),
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s1,
            StrataSpacing.s1,
            StrataSpacing.s2,
            StrataSpacing.s1,
          ),
          child: Row(
            children: [
              if (back != null)
                IconButton(
                  tooltip: l10n.backToNotes,
                  icon: const BackButtonIcon(),
                  onPressed: back,
                )
              else
                const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      note.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: text.titleSmall.copyWith(
                        color: colors.text,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    NoteStatusLabel(
                      status: controller.status,
                      pendingOps: note.sync_.pendingOps,
                    ),
                  ],
                ),
              ),
              if (controller.isDirty)
                FilledButton(
                  onPressed: controller.isSaving ? null : save,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(64, StrataLayout.minTouchTarget),
                  ),
                  child: Text(context.l10n.actionSave),
                ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(
            StrataSpacing.s4,
            0,
            StrataSpacing.s4,
            StrataSpacing.s2,
          ),
          child: Semantics(
            container: true,
            explicitChildNodes: true,
            label: l10n.noteViewsLabel,
            child: SegmentedButton<CompactNoteTab>(
              showSelectedIcon: false,
              segments: [
                ButtonSegment(
                  value: CompactNoteTab.note,
                  label: Text(l10n.tabNote),
                ),
                ButtonSegment(
                  value: CompactNoteTab.links,
                  label: Text(l10n.tabLinks),
                ),
                ButtonSegment(
                  value: CompactNoteTab.history,
                  label: Text(l10n.tabHistory),
                ),
              ],
              selected: {_tab},
              onSelectionChanged: (selection) =>
                  setState(() => _tab = selection.single),
            ),
          ),
        ),
        Expanded(child: body),
      ],
    );
  }
}
