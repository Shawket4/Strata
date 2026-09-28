import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
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
    this.onOpenDuplicate,
    this.onOpenLocalMap,
  });

  /// Opens a note by ID.
  final ValueChanged<String>? onOpenNote;

  /// Opens a wikilink's or citation's target (the note and block or heading
  /// the core resolved).
  final OpenNoteAt? onOpenLink;

  /// Opens the conflict screen for an op ID.
  final ValueChanged<String>? onOpenConflict;

  /// Opens the "Already exists" prompt of a duplicate-flagged op.
  final ValueChanged<String>? onOpenDuplicate;

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

VoidCallback? _duplicate(NoteView note, NoteActions actions) {
  final opId = note.sync_.duplicateOpId;
  final open = actions.onOpenDuplicate;
  if (opId == null || open == null) return null;
  return () => open(opId);
}

/// The banners above the note: the conflict hand-off, the "Already exists"
/// prompt of a duplicate-flagged note, and the block a link pointed at (the
/// editor's caret is put on it).
List<Widget> _banners(
  NoteView note,
  NoteActions actions,
  String? anchor,
  NoteEditorController controller,
) => [
  if (note.sync_.kind == NoteSyncKind.conflict) ...[
    NoteConflictBanner(onResolve: _conflict(note, actions)),
    const SizedBox(height: StrataSpacing.s4),
  ],
  if (note.sync_.kind == NoteSyncKind.duplicate) ...[
    NoteDuplicateBanner(onReview: _duplicate(note, actions)),
    const SizedBox(height: StrataSpacing.s4),
  ],
  if (anchor != null) ...[
    LinkedBlockCard(noteId: note.id, anchor: anchor, controller: controller),
    const SizedBox(height: StrataSpacing.s4),
  ],
];

/// "Created 18 Sep · edited today 14:31 by Shawket · 214 words", from the
/// core's labels.
class NoteMetaLine extends StatelessWidget {
  /// Creates the line for [note].
  const new({required this.note, super.key});

  /// The note.
  final NoteView note;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final style = context.strataText.caption.copyWith(color: colors.text2);
    final created = note.createdLabel;
    final edited = note.editedLabel;
    final by = note.editedBy;
    final parts = [
      if (created != null) l10n.metaCreated(date: created),
      if (edited != null && by == null) l10n.metaEdited(date: edited),
      if (edited != null && by != null)
        l10n.metaEditedBy(date: edited, name: by),
      l10n.wordCount(count: note.wordCount),
    ];
    return Wrap(
      spacing: StrataSpacing.s2,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        for (final (i, part) in parts.indexed) ...[
          if (i > 0) ExcludeSemantics(child: Text('·', style: style)),
          Text(part, style: style),
        ],
      ],
    );
  }
}

/// Pin to / unpin from the sidebar (`pin_note`, this device).
class PinNoteButton extends ConsumerWidget {
  /// Creates the button for [note].
  const new({required this.note, super.key});

  /// The note.
  final NoteView note;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    return IconButton(
      tooltip: note.pinned ? l10n.unpinNote : l10n.pinNote,
      isSelected: note.pinned,
      icon: const Icon(Icons.push_pin_outlined),
      selectedIcon: const Icon(Icons.push_pin),
      color: colors.text2,
      onPressed: () => unawaited(
        ref.read(coreApiProvider).pinNote(id: note.id, pinned: !note.pinned),
      ),
    );
  }
}

/// The block a link or citation pointed at (`#^id` or a heading), as the
/// core resolves it (`resolve_citation`): its heading and text in its own
/// direction, or that it is gone. With a [controller], the editor's caret is
/// put at the block (the core's `offset`) once it is resolved.
class LinkedBlockCard extends ConsumerStatefulWidget {
  /// Creates the card for [anchor] in [noteId].
  const new({
    required this.noteId,
    required this.anchor,
    super.key,
    this.controller,
  });

  /// The note.
  final String noteId;

  /// The block ID (without `^`) or heading.
  final String anchor;

  /// The note's editor, whose caret goes to the block.
  final NoteEditorController? controller;

  @override
  ConsumerState<LinkedBlockCard> createState() => _LinkedBlockCardState();
}

class _LinkedBlockCardState extends ConsumerState<LinkedBlockCard> {
  (String, int)? _revealed;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final anchor = widget.anchor;
    final preview = ref
        .watch(resolveCitationProvider(widget.noteId, anchor))
        .value;
    final offset = preview?.offset;
    final controller = widget.controller;
    if (controller != null && offset != null && _revealed != (anchor, offset)) {
      _revealed = (anchor, offset);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) controller.revealOffset(offset);
      });
    }
    if (preview == null) return const SizedBox.shrink();
    final block = preview.blockText;
    final heading = preview.heading;
    return Semantics(
      container: true,
      label: l10n.linkedBlockTitle,
      explicitChildNodes: true,
      child: Container(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        decoration: BoxDecoration(
          color: colors.accentTint,
          borderRadius: StrataRadii.inputRadius,
          border: BorderDirectional(
            start: BorderSide(color: colors.accent, width: 3),
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Wrap(
              spacing: StrataSpacing.s2,
              children: [
                Text(
                  l10n.linkedBlockTitle,
                  style: text.caption
                      .withWeight(FontWeight.w600)
                      .copyWith(color: colors.accentText),
                ),
                Text(
                  anchor,
                  textDirection: TextDirection.ltr,
                  style: text.monoSmall.copyWith(color: colors.text2),
                ),
              ],
            ),
            if (heading != null)
              Text(heading, style: text.bodySmall.withWeight(FontWeight.w600)),
            Text(
              block ?? l10n.linkedBlockMissing,
              textDirection: block == null
                  ? null
                  : textDirectionOf(preview.blockDir),
              textAlign: TextAlign.start,
              style: text.bodySmall.copyWith(
                color: block == null ? colors.text2 : colors.text,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A note the core flagged as a possible duplicate (`NoteSyncKind.duplicate`):
/// "Review" opens the "Already exists" prompt.
class NoteDuplicateBanner extends StatelessWidget {
  /// Creates the banner.
  const new({required this.onReview, super.key});

  /// Opens the prompt.
  final VoidCallback? onReview;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      container: true,
      liveRegion: true,
      child: Container(
        padding: const EdgeInsetsDirectional.fromSTEB(
          StrataSpacing.s4,
          StrataSpacing.s3,
          StrataSpacing.s2,
          StrataSpacing.s2,
        ),
        decoration: BoxDecoration(
          color: colors.warningTint,
          border: BorderDirectional(
            start: BorderSide(color: colors.warning, width: 3),
          ),
          borderRadius: StrataRadii.inputRadius,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.duplicateBannerTitle,
              style: text.bodySmall.copyWith(
                color: colors.warningText,
                fontWeight: FontWeight.w600,
              ),
            ),
            Text(
              l10n.duplicateBannerMessage,
              style: text.caption.copyWith(color: colors.warningText),
            ),
            Align(
              alignment: AlignmentDirectional.centerEnd,
              child: TextButton(
                onPressed: onReview,
                style: TextButton.styleFrom(
                  foregroundColor: colors.warningText,
                  minimumSize: const Size(64, StrataLayout.minTouchTarget),
                ),
                child: Text(l10n.duplicateReview),
              ),
            ),
          ],
        ),
      ),
    );
  }
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
    this.anchor,
  });

  /// The note.
  final String noteId;

  /// The block or heading a link pointed at.
  final String? anchor;

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
          anchor: anchor,
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
    required this.anchor,
  });

  final NoteView note;
  final NoteEditorController controller;
  final bool contextVisible;
  final VoidCallback onToggleContext;
  final NoteActions actions;
  final String? anchor;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final save = saveNote(context, controller);
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
                        label: note.sync_.label,
                      ),
                      if (note.versionLabel case final version?)
                        Text(
                          version,
                          style: text.monoSmall.copyWith(color: colors.text2),
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
                LivePreviewToggle(controller: controller),
                PinNoteButton(note: note),
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
                          textDirection: textDirectionOf(note.titleDir),
                          textAlign: TextAlign.start,
                          style: text.display.copyWith(
                            color: colors.text,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      const SizedBox(height: StrataSpacing.s1),
                      NoteMetaLine(note: note),
                      const SizedBox(height: StrataSpacing.s4),
                      ..._banners(note, actions, anchor, controller),
                      PropertiesPanel(
                        note: note,
                        onOpenNote: actions.onOpenNote,
                        onOpenCitation: actions.onOpenLink,
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
    this.anchor,
  });

  /// The note.
  final String noteId;

  /// The block or heading a link pointed at.
  final String? anchor;

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
                          onOpenCitation: actions.onOpenLink,
                        ),
                        const SizedBox(height: StrataSpacing.s4),
                        Semantics(
                          header: true,
                          child: Text(
                            note.title,
                            textDirection: textDirectionOf(note.titleDir),
                            textAlign: TextAlign.start,
                            style: text.display.copyWith(
                              color: colors.text,
                              fontSize: 26,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        NoteMetaLine(note: note),
                        const SizedBox(height: StrataSpacing.s3),
                        ..._banners(note, actions, widget.anchor, controller),
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
          FormattingToolbar(
            controller: controller,
            onOpenLink: actions.onOpenLink,
            onHideKeyboard: _editorFocus.unfocus,
          ),
        ],
      ),
      CompactNoteTab.links => ListView(
        key: const PageStorageKey('links'),
        primary: false,
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [
          PropertiesPanel(
            note: note,
            onOpenNote: actions.onOpenNote,
            onOpenCitation: actions.onOpenLink,
          ),
          const SizedBox(height: StrataSpacing.s4),
          BacklinksSection(
            groups: note.backlinks,
            onOpenNote: actions.onOpenNote,
          ),
          const SizedBox(height: StrataSpacing.s3),
          LocalGraphSlot(
            noteId: note.id,
            onOpenMap: openMap == null ? null : () => openMap(note.id),
          ),
        ],
      ),
      CompactNoteTab.history => ListView(
        key: const PageStorageKey('history'),
        primary: false,
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [HistorySection(note: note)],
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
                    Wrap(
                      spacing: StrataSpacing.s2,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
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
                FilledButton(
                  onPressed: controller.isSaving ? null : save,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(64, StrataLayout.minTouchTarget),
                  ),
                  child: Text(context.l10n.actionSave),
                ),
              PinNoteButton(note: note),
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
                  label: Text(l10n.linksTab(count: note.backlinkCount)),
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
