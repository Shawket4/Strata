/// Strata notes feature: the notes list and the note view (editor,
/// Properties panel with typed relation chips, backlinks by relation type,
/// history, local graph slot), adaptive per size class (PLAN §11). UI only;
/// view-models and intents come from the Rust core (PLAN L15).
library;

export 'src/context/note_context.dart'
    show BacklinksSection, HistorySection, LocalGraphSlot, NoteContextPanel;
export 'src/detail/note_detail.dart'
    show CompactNotePage, NoteActions, NoteDetailPane;
export 'src/detail/properties_panel.dart' show PropertiesPanel;
export 'src/detail/relation_actions.dart'
    show AiRelationCard, NoteRelationChip, showAiRelationSheet;
export 'src/generated/notes_localizations.dart' show NotesLocalizations;
export 'src/list/notes_list_pane.dart' show NoteRow, NotesListPane;
export 'src/notes_scope.dart' show NotesLocalizationsScope;
export 'src/notes_screen.dart' show NotesScreen;
