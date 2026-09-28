/// Strata editor feature: the markdown note editor (PLAN §11 screen 4, D2 =
/// b) on `super_editor`, with a byte-exact source serialiser, the core's
/// highlight hints (live preview hides markers off the caret line),
/// task-line checkboxes, wikilinks, and the core's `[[` / `@` / `#` / `^`
/// completions. UI only; view-models, hints and intents come from the Rust
/// core (PLAN L15).
library;

export 'src/editor/completions_panel.dart'
    show
        CompletionTile,
        CompletionsCaretLayer,
        CompletionsPopup,
        completionGlyph;
export 'src/editor/editor_chrome.dart'
    show
        FormattingToolbar,
        LivePreviewToggle,
        NoteConflictBanner,
        NoteStatusLabel;
export 'src/editor/hint_styling.dart' show hiddenMarkerAttribution;
export 'src/editor/note_editor_controller.dart'
    show NoteEditStatus, NoteEditorController;
export 'src/editor/strata_note_editor.dart' show StrataNoteEditor;
export 'src/generated/editor_localizations.dart' show EditorLocalizations;
export 'src/note_editor_screen.dart'
    show
        NoteEditorHeader,
        NoteEditorPane,
        NoteEditorScreen,
        NoteLoadError,
        saveNote;
export 'src/note_editor_session.dart'
    show EditorLocalizationsScope, NoteEditorSession, NoteSessionBuilder;
export 'src/source/markdown_source.dart' show MarkdownSource, SourceLine;
export 'src/source/source_document.dart'
    show LineHints, LineSpan, documentOf, linesOf;
