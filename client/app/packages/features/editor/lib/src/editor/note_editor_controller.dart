import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:strata_editor/src/source/markdown_source.dart';
import 'package:strata_editor/src/source/source_document.dart';
import 'package:strata_state/strata_state.dart';
import 'package:super_editor/super_editor.dart';

/// Which autocomplete the characters before the caret ask for.
enum TriggerKind {
  /// `[[query` — note links (core search).
  wikilink,

  /// `[[Note#^` — block reference picker.
  blockReference,

  /// `@query` — people and companies (core directory).
  mention,

  /// `#query` — tags.
  tag,
}

/// The token being completed: its kind, the line it is on and its range
/// (the trigger characters included), and the text typed after the trigger.
@immutable
final class EditorTrigger {
  /// Creates a trigger.
  const new({
    required this.kind,
    required this.nodeId,
    required this.start,
    required this.end,
    required this.query,
  });

  /// Kind.
  final TriggerKind kind;

  /// The line (paragraph node) holding the token.
  final String nodeId;

  /// Start of the token (the trigger characters).
  final int start;

  /// End of the token (the caret).
  final int end;

  /// What the user typed after the trigger characters.
  final String query;

  @override
  bool operator ==(Object other) =>
      other is EditorTrigger &&
      other.kind == kind &&
      other.nodeId == nodeId &&
      other.start == start &&
      other.end == end &&
      other.query == query;

  @override
  int get hashCode => Object.hash(kind, nodeId, start, end, query);

  @override
  String toString() => 'EditorTrigger($kind, "$query")';
}

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
}

/// The editing session of one note: the `super_editor` [Editor] over the
/// note's markdown source (see `MarkdownSource`), the core's hints placed on
/// its lines, and the intents the editor forwards (save, task toggles,
/// mentions).
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
  EditorTrigger? _trigger;
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

  /// The autocomplete token at the caret, if any.
  EditorTrigger? get trigger => _trigger;

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
      NoteSyncKind.pending || NoteSyncKind.duplicate => NoteEditStatus.pending,
      NoteSyncKind.conflict => NoteEditStatus.conflict,
    };
  }

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

  void _load(NoteView note) {
    final source = MarkdownSource.parse(note.content, note.hints);
    _frontmatter = source.frontmatter;
    _newline = source.newline;
    _document = documentOf(source);
    _composer = MutableDocumentComposer();
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
    _trigger = null;
    _applyHints(note.content, note.hints);
  }

  void _onEdit(List<EditEvent> changes) {
    final content = _joined();
    final changed = content != _content;
    _content = content;
    _trigger = _detectTrigger();
    if (changed) unawaited(_refreshHints());
    notifyListeners();
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

  /// Saves the note: `updateNote` with the note's ID and the full markdown
  /// (latest frontmatter + edited body). Does nothing when nothing changed.
  /// Returns whether the core accepted it.
  Future<bool> save() async {
    final note = _note;
    if (note == null || !isDirty || _saving) return true;
    _saving = true;
    notifyListeners();
    try {
      await core.updateNote(id: note.id, content: _content);
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
  // Autocomplete
  // ---------------------------------------------------------------------------

  EditorTrigger? _detectTrigger() {
    final selection = _composer.selection;
    if (selection == null || !selection.isCollapsed) return null;
    final position = selection.extent.nodePosition;
    if (position is! TextNodePosition) return null;
    final node = _document.getNodeById(selection.extent.nodeId);
    if (node is! TextNode) return null;
    final text = node.text.toPlainText();
    final caret = position.offset.clamp(0, text.length);
    final before = text.substring(0, caret);
    final open = before.lastIndexOf('[[');
    if (open >= 0 && !before.substring(open).contains(']]')) {
      final query = before.substring(open + 2);
      return EditorTrigger(
        kind: query.contains('#^')
            ? TriggerKind.blockReference
            : TriggerKind.wikilink,
        nodeId: node.id,
        start: open,
        end: caret,
        query: query,
      );
    }
    for (var i = before.length - 1; i >= 0; i--) {
      final char = before[i];
      if (char.trim().isEmpty) return null;
      if (char != '@' && char != '#') continue;
      if (i > 0 && before[i - 1].trim().isNotEmpty) return null;
      return EditorTrigger(
        kind: char == '@' ? TriggerKind.mention : TriggerKind.tag,
        nodeId: node.id,
        start: i,
        end: caret,
        query: before.substring(i + 1),
      );
    }
    return null;
  }

  /// Closes the autocomplete panel until the next edit.
  void dismissTrigger() {
    if (_trigger == null) return;
    _trigger = null;
    notifyListeners();
  }

  /// Completes a `[[` link with [title] (the note title the core's search
  /// returned) and closes it.
  void completeWikilink(String title) {
    final trigger = _trigger;
    if (trigger == null) return;
    _replace(trigger.nodeId, trigger.start + 2, trigger.end, '$title]]');
  }

  /// Replaces the `@query` token with a link to [entity] and asks the core
  /// to add the entity to the note's `people:` or `companies:` relation
  /// ([relationKey]).
  Future<void> completeMention(
    DirectoryItem entity, {
    required String relationKey,
  }) async {
    final trigger = _trigger;
    final note = _note;
    if (trigger == null || note == null) return;
    _replace(trigger.nodeId, trigger.start, trigger.end, '[[${entity.title}]]');
    await core.addRelation(
      srcId: note.id,
      dstId: entity.id,
      relType: relationKey,
    );
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
    _trigger = null;
    notifyListeners();
  }

  // ---------------------------------------------------------------------------
  // Formatting (typing markdown characters on the user's behalf)
  // ---------------------------------------------------------------------------

  /// Inserts [text] at the caret (e.g. `[[`, `@`, `#` from the toolbar).
  void insertAtCaret(String text) {
    final caret = _caret();
    if (caret == null) return;
    _replace(caret.$1, caret.$2, caret.$2, text);
    _trigger = _detectTrigger();
    notifyListeners();
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

  /// The raw link source (`[[…]]` / `![[…]]`) of the wikilink or embed hint
  /// at [offset] of line [nodeId], if any.
  String? linkAt(String nodeId, int offset) {
    final span =
        _lineHints.at(nodeId, offset, HintKind.wikiLink) ??
        _lineHints.at(nodeId, offset, HintKind.embed);
    if (span == null) return null;
    final node = _document.getNodeById(nodeId);
    if (node is! TextNode) return null;
    return node.text.toPlainText().substring(span.start, span.end);
  }

  /// The wikilink under the caret, if any.
  String? get linkAtCaret {
    final caret = _caret();
    return caret == null ? null : linkAt(caret.$1, caret.$2);
  }

  @override
  void dispose() {
    _editor?.dispose();
    super.dispose();
  }
}
