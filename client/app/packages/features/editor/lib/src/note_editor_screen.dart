import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_editor/src/editor/editor_chrome.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/editor/strata_note_editor.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_editor/src/note_editor_session.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the editor feature: note [noteId] in the markdown editor
/// (PLAN §11 screen 4, the editor part; the notes feature adds the
/// properties panel, backlinks and history around it).
///
/// Loading, error and "not found" states come from the note stream; the
/// status line and the conflict banner from the note's sync state. Saving
/// (⌘/Ctrl-S or the Save button) calls `updateNote` with the note's ID and
/// the full markdown.
class NoteEditorScreen extends StatelessWidget {
  /// Creates the editor screen for [noteId].
  const new({
    required this.noteId,
    super.key,
    this.onBack,
    this.onOpenLink,
    this.onOpenConflict,
  });

  /// The note to edit.
  final String noteId;

  /// Leaves the screen (shows a back button when set).
  final VoidCallback? onBack;

  /// Opens a wikilink's target (note and anchor, resolved by the core).
  final OpenNoteAt? onOpenLink;

  /// Opens the conflict screen for an op ID (sync feature).
  final ValueChanged<String>? onOpenConflict;

  /// The icon that represents this feature.
  static const IconData icon = Icons.edit_note;

  @override
  Widget build(BuildContext context) => NoteEditorSession(
    noteId: noteId,
    builder: (context, screen, controller) => Material(
      color: context.strataColors.surface,
      child: switch (screen) {
        AsyncData(:final value) when value.note == null => _Centered(
          child: StrataEmptyState(
            icon: Icons.search_off,
            title: EditorLocalizations.of(context).noteNotFoundTitle,
            message: EditorLocalizations.of(context).noteNotFoundMessage,
          ),
        ),
        AsyncData(:final value) => NoteEditorPane(
          note: value.note!,
          controller: controller,
          onBack: onBack,
          onOpenLink: onOpenLink,
          onOpenConflict: onOpenConflict,
        ),
        AsyncError() => _Centered(child: NoteLoadError(noteId: noteId)),
        _ => const Center(child: CircularProgressIndicator()),
      },
    ),
  );
}

class _Centered extends StatelessWidget {
  const new({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Center(child: SingleChildScrollView(child: child));
}

/// The "couldn't open this note" state with a retry that re-subscribes to
/// the note stream.
class NoteLoadError extends ConsumerWidget {
  /// Creates the error state for [noteId].
  const new({required this.noteId, super.key});

  /// The note.
  final String noteId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = EditorLocalizations.of(context);
    return StrataEmptyState(
      icon: Icons.error_outline,
      title: l10n.noteLoadErrorTitle,
      message: l10n.noteLoadErrorMessage,
      action: StrataAction(
        label: context.l10n.actionRetry,
        icon: Icons.refresh,
        onPressed: () => ref.invalidate(noteProvider(noteId)),
      ),
    );
  }
}

/// The editor pane of a loaded note: header (title, path, status, version,
/// Save, live preview), conflict banner, the editor (completions pop up at
/// the caret) and, on compact, the formatting toolbar.
class NoteEditorPane extends StatefulWidget {
  /// Creates the pane.
  const new({
    required this.note,
    required this.controller,
    super.key,
    this.onBack,
    this.onOpenLink,
    this.onOpenConflict,
  });

  /// The note (latest view).
  final NoteView note;

  /// The editing session.
  final NoteEditorController controller;

  /// Leaves the screen.
  final VoidCallback? onBack;

  /// Opens a wikilink's target.
  final OpenNoteAt? onOpenLink;

  /// Opens the conflict screen.
  final ValueChanged<String>? onOpenConflict;

  @override
  State<NoteEditorPane> createState() => _NoteEditorPaneState();
}

class _NoteEditorPaneState extends State<NoteEditorPane> {
  final FocusNode _editorFocus = FocusNode(debugLabel: 'note editor');

  @override
  void dispose() {
    _editorFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final note = widget.note;
    final compact = SizeClass.of(context) == SizeClass.compact;
    final save = saveNote(context, controller);
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyS, meta: true): save,
        const SingleActivator(LogicalKeyboardKey.keyS, control: true): save,
      },
      child: Column(
        children: [
          NoteEditorHeader(
            note: note,
            controller: controller,
            onBack: widget.onBack,
            onSave: save,
            trailing: [if (!compact) LivePreviewToggle(controller: controller)],
          ),
          Expanded(
            child: CustomScrollView(
              slivers: [
                if (note.sync_.kind == NoteSyncKind.conflict)
                  SliverPadding(
                    padding: const EdgeInsets.fromLTRB(
                      StrataSpacing.s4,
                      StrataSpacing.s3,
                      StrataSpacing.s4,
                      0,
                    ),
                    sliver: SliverToBoxAdapter(
                      child: NoteConflictBanner(
                        onResolve: _conflictAction(note, widget.onOpenConflict),
                      ),
                    ),
                  ),
                SliverPadding(
                  padding: EdgeInsets.symmetric(
                    horizontal: compact ? StrataSpacing.s5 : StrataSpacing.s8,
                    vertical: StrataSpacing.s5,
                  ),
                  sliver: StrataNoteEditor(
                    controller: controller,
                    focusNode: _editorFocus,
                    onOpenLink: widget.onOpenLink,
                    onSave: save,
                  ),
                ),
              ],
            ),
          ),
          if (compact)
            FormattingToolbar(
              controller: controller,
              onOpenLink: widget.onOpenLink,
              onHideKeyboard: _editorFocus.unfocus,
            ),
        ],
      ),
    );
  }
}

VoidCallback? _conflictAction(
  NoteView note,
  ValueChanged<String>? onOpenConflict,
) {
  final opId = note.sync_.conflictOpId;
  if (opId == null || onOpenConflict == null) return null;
  return () => onOpenConflict(opId);
}

/// The save action of [controller]: saves and reports a failure in a snack
/// bar (the text stays in the editor).
VoidCallback saveNote(BuildContext context, NoteEditorController controller) =>
    () => unawaited(
      controller.save().then((ok) {
        if (ok || !context.mounted) return;
        ScaffoldMessenger.maybeOf(context)?.showSnackBar(
          SnackBar(content: Text(EditorLocalizations.of(context).saveFailed)),
        );
      }),
    );

/// The editor header: back (optional), title and path, status line and a
/// Save button while there is unsaved text.
class NoteEditorHeader extends StatelessWidget {
  /// Creates the header.
  const new({
    required this.note,
    required this.controller,
    required this.onSave,
    super.key,
    this.onBack,
    this.trailing = const [],
  });

  /// The note.
  final NoteView note;

  /// The editing session.
  final NoteEditorController controller;

  /// Saves.
  final VoidCallback onSave;

  /// Leaves the screen.
  final VoidCallback? onBack;

  /// Extra actions at the end (context panel toggle, map, …).
  final List<Widget> trailing;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final back = onBack;
    return Container(
      constraints: const BoxConstraints(minHeight: 56),
      padding: const EdgeInsetsDirectional.fromSTEB(
        StrataSpacing.s1,
        StrataSpacing.s1,
        StrataSpacing.s2,
        StrataSpacing.s1,
      ),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: colors.border)),
      ),
      child: Row(
        children: [
          if (back != null)
            IconButton(
              tooltip: context.l10n.actionBack,
              icon: const BackButtonIcon(),
              onPressed: back,
            )
          else
            const SizedBox(width: StrataSpacing.s5),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Semantics(
                  header: true,
                  container: true,
                  child: Text(
                    note.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: text.titleSmall.copyWith(color: colors.text),
                  ),
                ),
                Wrap(
                  spacing: StrataSpacing.s3,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      note.path,
                      style: text.monoSmall.copyWith(color: colors.text2),
                    ),
                    NoteStatusLabel(
                      status: controller.status,
                      label: note.sync_.label,
                    ),
                    if (note.versionLabel case final version?)
                      Text(
                        version,
                        style: text.monoSmall.copyWith(color: colors.text2),
                      ),
                  ],
                ),
              ],
            ),
          ),
          if (controller.isDirty)
            Padding(
              padding: const EdgeInsetsDirectional.only(
                start: StrataSpacing.s2,
              ),
              child: FilledButton(
                onPressed: controller.isSaving ? null : onSave,
                style: FilledButton.styleFrom(
                  minimumSize: const Size(64, StrataLayout.minTouchTarget),
                ),
                child: Text(context.l10n.actionSave),
              ),
            ),
          ...trailing,
        ],
      ),
    );
  }
}
