import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:strata_editor/src/source/markdown_source.dart';
import 'package:strata_editor/src/source/source_document.dart';
import 'package:strata_state/strata_state.dart';
import 'package:super_editor/super_editor.dart';

/// What the note's status line shows, mapped 1:1 from the note's sync state
/// and whether the editor holds unsaved text.
enum NoteEditStatus {
  /// No local changes.
  saved,

  /// The editor holds text that was not saved yet.
  unsaved,

  /// Saved on this device; ops wait to sync.
  pending,

  /// An edit conflicts with the server.
  conflict,

  /// Creating the note found an existing one ("Already exists").
  duplicate,
}

/// The editing session of one note: the `super_editor` [Editor] over the
/// note's markdown source (see `MarkdownSource`), the core's hints placed on
/// its lines, the core's completions at the caret, and the intents the
/// editor forwards (save, task toggles, mentions).
///
/// Holds only ephemeral editing state (the text being typed); the note
/// itself always comes from the core (PLAN L15).
final class NoteEditorController extends ChangeNotifier {
  /// Creates a controller that forwards intents to [core].
  new({required this.core});

  /// The Rust core.
  final CoreApi core;

  NoteView? _note;
  String _frontmatter = '';
  String _newline = '\n';
  late MutableDocument _document;
  late MutableDocumentComposer _composer;
  Editor? _editor;
  LineHints _lineHints = const LineHints();
  List<EditorHint> _hints = const [];
  String _hintedContent = '';
  String _content = '';
  Completions? _completions;
  int _completionRequest = 0;
  String? _caretNodeId;
  bool _livePreview = true;
  bool _saving = false;
  int _generation = 0;

  /// The note shown (the latest the core streamed).
  NoteView? get note => _note;

  /// The editor of the current document (replaced when the core's content
  /// changes while nothing is being edited).
  Editor? get editor => _editor;

  /// The document being edited.
  MutableDocument get document => _document;

  /// The selection holder.
  MutableDocumentComposer get composer => _composer;

  /// The core's hints placed on the document's lines.
  LineHints get lineHints => _lineHints;

  /// Increments whenever [lineHints] or the note's tasks change (the
  /// editor's style phase re-runs).
  int get generation => _generation;

  /// The markdown the editor holds: the latest frontmatter from the core,
  /// then the edited body.
  String get content => _content;

  /// Whether the editor holds text the core does not have.
  bool get isDirty => _note != null && _content != _note!.content;

  /// Whether a save is running.
  bool get isSaving => _saving;

  /// The core's completions at the caret (`editor_completions`), while
  /// there is something to complete.
  Completions? get completions => _completions;

  /// The line holding the caret (its markdown markers stay visible).
  String? get caretNodeId => _caretNodeId;

  /// Live preview: markdown markers (`**`, `#`, `[[`…) are hidden on every
  /// line but the caret's. Off: the source is shown as typed.
  bool get livePreview => _livePreview;

  /// Turns live preview on or off.
  set livePreview(bool value) {
    if (value == _livePreview) return;
    _livePreview = value;
    _generation++;
    notifyListeners();
  }

  /// The status line state.
  NoteEditStatus get status {
    final note = _note;
    if (note == null) return NoteEditStatus.saved;
    if (note.sync_.kind == NoteSyncKind.conflict) {
      return NoteEditStatus.conflict;
    }
    if (isDirty) return NoteEditStatus.unsaved;
    return switch (note.sync_.kind) {
      NoteSyncKind.synced => NoteEditStatus.saved,
      NoteSyncKind.pending => NoteEditStatus.pending,
      NoteSyncKind.duplicate => NoteEditStatus.duplicate,
      NoteSyncKind.conflict => NoteEditStatus.conflict,
    };
  }

  String? _baseVersion;

  /// Shows [note], the latest view the core streamed.
  ///
  /// The document is rebuilt from the note's content when nothing is being
  /// edited (or on the first view); while the editor holds unsaved text the
  /// body is kept and only the frontmatter (which the editor never edits)
  /// follows the core.
  void show(NoteView note) {
    final first = _note == null || _note!.id != note.id;
    final dirty = !first && isDirty;
    _note = note;
    // The version the shown text was loaded from: the base of the next save
    // (the core 3-way merges a stale base, D19). Kept while text is unsaved.
    if (!dirty) _baseVersion = note.contentVersion;
    if (first || (!dirty && note.content != _content)) {
      _load(note);
    } else {
      final end = MarkdownSource.frontmatterEnd(note.content, note.hints);
      _frontmatter = note.content.substring(0, end);
      _content = _joined();
      if (_content == note.content) {
        _applyHints(note.content, note.hints);
      } else {
        unawaited(_refreshHints());
      }
    }
    _generation++;
    notifyListeners();
  }

  String _joined() =>
      '$_frontmatter${MarkdownSource.joinLines(linesOf(_document), _newline)}';

  void _load(NoteView note) => _loadContent(note.content, note.hints);

  void _loadContent(String content, List<EditorHint> hints) {
    final source = MarkdownSource.parse(content, hints);
    _frontmatter = source.frontmatter;
    _newline = source.newline;
    _document = documentOf(source);
    _composer = MutableDocumentComposer()
      ..selectionNotifier.addListener(_onSelection);
    _caretNodeId = null;
    _editor?.dispose();
    _editor = Editor(
      editables: {Editor.documentKey: _document, Editor.composerKey: _composer},
      requestHandlers: List.from(defaultRequestHandlers),
      // No content conversions: the editor edits markdown source, so `# `,
      // `- `, `--` and URLs stay the characters the user typed.
      reactionPipeline: [],
      isHistoryEnabled: true,
    )..addListener(FunctionalEditListener(_onEdit));
    _content = source.content;
    _completions = null;
    _applyHints(content, hints);
  }

  void _onEdit(List<EditEvent> changes) {
    final content = _joined();
    final changed = content != _content;
    _content = content;
    if (changed) {
      unawaited(_refreshHints());
      unawaited(_refreshCompletions());
    }
    notifyListeners();
  }

  void _onSelection() {
    final node = _composer.selection?.extent.nodeId;
    if (node != _caretNodeId) {
      // The markers of the line the caret left hide, the new line's show.
      _caretNodeId = node;
      _generation++;
      notifyListeners();
    }
    if (_completions != null) unawaited(_refreshCompletions());
  }

  Future<void> _refreshHints() async {
    final content = _content;
    if (content == _hintedContent) return;
    try {
      final hints = await core.editorHints(content: content);
      if (content != _content) return;
      _applyHints(content, hints);
      _generation++;
      notifyListeners();
    } on Object {
      // Highlighting is cosmetic; the next edit asks again.
    }
  }

  void _applyHints(String content, List<EditorHint> hints) {
    _hints = hints;
    _hintedContent = content;
    _lineHints = LineHints.place(
      document: _document,
      lines: linesOf(_document),
      newline: _newline,
      bodyOffset: _frontmatter.length,
      hints: hints,
    );
  }

  List<int> _lineStarts() => lineStartsOf(
    _document,
    linesOf(_document),
    newline: _newline,
    bodyOffset: _frontmatter.length,
  );

  /// The caret as a UTF-16 offset into [content] (`null` without a caret).
  int? get caretOffset {
    final caret = _caret();
    if (caret == null) return null;
    final index = _document.getNodeIndexById(caret.$1);
    if (index < 0) return null;
    return _lineStarts()[index] + caret.$2;
  }

  /// The line and offset in it of [offset] into [content] (`null`: inside
  /// the frontmatter or past the end).
  (String, int)? positionOf(int offset) {
    final starts = _lineStarts();
    final lines = linesOf(_document);
    final nodes = _document.toList();
    for (var i = 0; i < nodes.length; i++) {
      final start = starts[i];
      if (offset >= start && offset <= start + lines[i].text.length) {
        return (nodes[i].id, offset - start);
      }
    }
    return null;
  }

  /// Puts the caret at [offset] into [content] (a citation's block, from the
  /// core's `CitationPreview.offset`) so the editor shows it. Returns whether
  /// the offset is in the editable body.
  bool revealOffset(int offset) {
    final at = positionOf(offset);
    final editor = _editor;
    if (at == null || editor == null) return false;
    editor.execute([
      ChangeSelectionRequest(
        DocumentSelection.collapsed(
          position: DocumentPosition(
            nodeId: at.$1,
            nodePosition: TextNodePosition(offset: at.$2),
          ),
        ),
        SelectionChangeType.placeCaret,
        SelectionReason.userInteraction,
      ),
    ]);
    return true;
  }

  /// The hints the current line placement was computed from.
  @visibleForTesting
  List<EditorHint> get hints => _hints;

  /// The task of the task line [nodeId] (`null`: not a task line, or the
  /// core does not know the task yet, e.g. a line typed but not saved).
  TaskItem? taskOf(String nodeId) {
    final id = _lineHints.taskIds[nodeId];
    final note = _note;
    if (id == null || note == null) return null;
    for (final task in note.tasks) {
      if (task.id == id) return task;
    }
    return null;
  }

  /// Saves the note: `updateNote` with the note's ID, the full markdown
  /// (latest frontmatter + edited body) and the version it was edited from.
  /// Does nothing when nothing changed. Returns whether the core accepted it.
  Future<bool> save() async {
    final note = _note;
    if (note == null || !isDirty || _saving) return true;
    _saving = true;
    notifyListeners();
    try {
      await core.updateNote(
        id: note.id,
        content: _content,
        baseVersion: _baseVersion,
      );
      return true;
    } on Object {
      return false;
    } finally {
      _saving = false;
      notifyListeners();
    }
  }

  /// Toggles the task of line [nodeId]: an open task is completed, a done or
  /// cancelled one reopened (the core writes the line). Unsaved text is saved
  /// first so the core applies the toggle to what the user sees.
  Future<void> toggleTask(String nodeId) async {
    final task = taskOf(nodeId);
    if (task == null) return;
    if (isDirty && !await save()) return;
    if (task.state == TaskState.open) {
      await core.completeTask(taskId: task.id);
    } else {
      await core.reopenTask(taskId: task.id);
    }
  }

  // ---------------------------------------------------------------------------
  // Completions (the core decides what the text at the caret asks for)
  // ---------------------------------------------------------------------------

  Future<void> _refreshCompletions() async {
    final note = _note;
    final cursor = caretOffset;
    final request = ++_completionRequest;
    if (note == null || cursor == null) {
      _setCompletions(null);
      return;
    }
    try {
      final completions = await core.editorCompletions(
        noteId: note.id,
        content: _content,
        cursor: cursor,
      );
      if (request != _completionRequest) return;
      _setCompletions(
        completions.kind == CompletionKind.none ? null : completions,
      );
    } on Object {
      // Completions are a convenience; the next edit asks again.
      if (request == _completionRequest) _setCompletions(null);
    }
  }

  void _setCompletions(Completions? completions) {
    if (completions == _completions) return;
    _completions = completions;
    notifyListeners();
  }

  /// Closes the completions until the next edit.
  void dismissCompletions() {
    _completionRequest++;
    _setCompletions(null);
  }

  /// Applies [item] of the current completions: a mention goes through the
  /// core (`insert_mention`: the link and the `people:` / `companies:`
  /// entry in one content change, saved at once); any other item replaces
  /// the typed range with the core's `insert_text`.
  Future<void> applyCompletion(CompletionItem item) async {
    final completions = _completions;
    final note = _note;
    if (completions == null || note == null) return;
    dismissCompletions();
    if (completions.kind == CompletionKind.mention) {
      final entity = item.targetId;
      if (entity == null) return;
      final edit = await core.insertMention(
        noteId: note.id,
        content: _content,
        start: completions.replaceStart,
        end: completions.replaceEnd,
        entityId: entity,
      );
      await _replaceContent(edit.content, edit.cursor);
      await save();
      return;
    }
    final start = positionOf(completions.replaceStart);
    final end = positionOf(completions.replaceEnd);
    if (start == null || end == null || start.$1 != end.$1) return;
    _replace(start.$1, start.$2, end.$2, item.insertText);
  }

  /// Replaces the whole markdown with [content] (a core edit that touches
  /// the frontmatter too) and places the caret at [cursor].
  Future<void> _replaceContent(String content, int cursor) async {
    final hints = await core.editorHints(content: content);
    _loadContent(content, hints);
    final at = positionOf(cursor);
    if (at != null) {
      _editor?.execute([
        ChangeSelectionRequest(
          DocumentSelection.collapsed(
            position: DocumentPosition(
              nodeId: at.$1,
              nodePosition: TextNodePosition(offset: at.$2),
            ),
          ),
          SelectionChangeType.placeCaret,
          SelectionReason.userInteraction,
        ),
      ]);
    }
    _generation++;
    notifyListeners();
  }

  void _replace(String nodeId, int start, int end, String text) {
    final editor = _editor;
    if (editor == null) return;
    DocumentPosition at(int offset) => DocumentPosition(
      nodeId: nodeId,
      nodePosition: TextNodePosition(offset: offset),
    );
    editor.execute([
      if (end > start)
        DeleteContentRequest(
          documentRange: DocumentRange(start: at(start), end: at(end)),
        ),
      InsertTextRequest(
        documentPosition: at(start),
        textToInsert: text,
        attributions: const {},
      ),
      ChangeSelectionRequest(
        DocumentSelection.collapsed(position: at(start + text.length)),
        SelectionChangeType.placeCaret,
        SelectionReason.userInteraction,
      ),
    ]);
    notifyListeners();
  }

  // ---------------------------------------------------------------------------
  // Formatting (typing markdown characters on the user's behalf)
  // ---------------------------------------------------------------------------

  /// Inserts [text] at the caret (e.g. `[[`, `@`, `#` from the toolbar);
  /// the core's completions follow.
  void insertAtCaret(String text) {
    final caret = _caret();
    if (caret == null) return;
    _replace(caret.$1, caret.$2, caret.$2, text);
  }

  /// Inserts [marker] at the start of the caret's line (`# `, `- `,
  /// `- [ ] `).
  void prefixLine(String marker) {
    final caret = _caret();
    final editor = _editor;
    if (caret == null || editor == null) return;
    final (nodeId, offset) = caret;
    editor.execute([
      InsertTextRequest(
        documentPosition: DocumentPosition(
          nodeId: nodeId,
          nodePosition: const TextNodePosition(offset: 0),
        ),
        textToInsert: marker,
        attributions: const {},
      ),
      ChangeSelectionRequest(
        DocumentSelection.collapsed(
          position: DocumentPosition(
            nodeId: nodeId,
            nodePosition: TextNodePosition(offset: offset + marker.length),
          ),
        ),
        SelectionChangeType.placeCaret,
        SelectionReason.userInteraction,
      ),
    ]);
  }

  /// Wraps the selection within one line in [marker] (`**`, `_`); with a
  /// collapsed caret inserts the pair and places the caret between.
  void wrapSelection(String marker) {
    final selection = _composer.selection;
    final editor = _editor;
    if (selection == null || editor == null) return;
    final base = selection.base;
    final extent = selection.extent;
    if (base.nodeId != extent.nodeId) return;
    final a = base.nodePosition;
    final b = extent.nodePosition;
    if (a is! TextNodePosition || b is! TextNodePosition) return;
    final start = a.offset < b.offset ? a.offset : b.offset;
    final end = a.offset < b.offset ? b.offset : a.offset;
    DocumentPosition at(int offset) => DocumentPosition(
      nodeId: base.nodeId,
      nodePosition: TextNodePosition(offset: offset),
    );
    editor.execute([
      InsertTextRequest(
        documentPosition: at(end),
        textToInsert: marker,
        attributions: const {},
      ),
      InsertTextRequest(
        documentPosition: at(start),
        textToInsert: marker,
        attributions: const {},
      ),
      ChangeSelectionRequest(
        start == end
            ? DocumentSelection.collapsed(position: at(start + marker.length))
            : DocumentSelection(
                base: at(start + marker.length),
                extent: at(end + marker.length),
              ),
        SelectionChangeType.placeCaret,
        SelectionReason.userInteraction,
      ),
    ]);
  }

  /// Undoes the last edit.
  void undo() => _editor?.undo();

  /// Redoes the last undone edit.
  void redo() => _editor?.redo();

  (String, int)? _caret() {
    final selection = _composer.selection;
    if (selection == null) return null;
    final position = selection.extent.nodePosition;
    if (position is! TextNodePosition) return null;
    return (selection.extent.nodeId, position.offset);
  }

  /// The wikilink or embed hint at [offset] of line [nodeId] whose target
  /// the core resolved (`target_id`, `target_anchor`), if any.
  LineSpan? linkAt(String nodeId, int offset) {
    final span =
        _lineHints.at(nodeId, offset, HintKind.wikiLink) ??
        _lineHints.at(nodeId, offset, HintKind.embed);
    return span?.targetId == null ? null : span;
  }

  /// The resolved wikilink under the caret, if any.
  LineSpan? get linkAtCaret {
    final caret = _caret();
    return caret == null ? null : linkAt(caret.$1, caret.$2);
  }

  @override
  void dispose() {
    _editor?.dispose();
    super.dispose();
  }
}
