// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'editor_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class EditorLocalizationsEn extends EditorLocalizations {
  EditorLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get editorScreenTitle => 'Note editor';

  @override
  String get editorBodyLabel => 'Note body';

  @override
  String get statusUnsaved => 'Unsaved changes';

  @override
  String get conflictBannerTitle => 'This note was also changed on the server';

  @override
  String get conflictBannerMessage =>
      'Your edit overlaps with the server\'s version. Resolve the conflict to keep syncing this note.';

  @override
  String get conflictResolve => 'Resolve';

  @override
  String get noteNotFoundTitle => 'Note not found';

  @override
  String get noteNotFoundMessage => 'It may have been deleted or moved.';

  @override
  String get noteLoadErrorTitle => 'Couldn\'t open this note';

  @override
  String get noteLoadErrorMessage =>
      'Something went wrong while reading this note on this device.';

  @override
  String get saveFailed => 'Couldn\'t save. Your text is still here.';

  @override
  String get toolbarLabel => 'Formatting';

  @override
  String get toolUndo => 'Undo';

  @override
  String get toolRedo => 'Redo';

  @override
  String get toolHeading => 'Heading';

  @override
  String get toolBold => 'Bold';

  @override
  String get toolItalic => 'Italic';

  @override
  String get toolBulletList => 'Bulleted list';

  @override
  String get toolChecklist => 'Checklist item';

  @override
  String get toolWikilink => 'Insert wikilink';

  @override
  String get toolMention => 'Mention a person or company';

  @override
  String get toolTag => 'Tag';

  @override
  String get toolHideKeyboard => 'Hide keyboard';

  @override
  String get toolOpenLink => 'Open link';

  @override
  String taskComplete({required String task}) {
    return 'Complete task: $task';
  }

  @override
  String taskReopen({required String task}) {
    return 'Reopen task: $task';
  }

  @override
  String get taskNotSynced => 'Task line (save to enable)';

  @override
  String get suggestionsLabel => 'Suggestions';

  @override
  String get suggestionsNone => 'No matches';

  @override
  String get suggestionsDismiss => 'Dismiss suggestions';

  @override
  String get statusBarMarkdown => 'Markdown';

  @override
  String get statusBarRtl => 'RTL · auto per paragraph';

  @override
  String get shortcutSave => 'Save';

  @override
  String get suggestionsKindWikiLink => 'Notes';

  @override
  String get suggestionsKindMention => 'People and companies';

  @override
  String get suggestionsKindTag => 'Tags';

  @override
  String get suggestionsKindBlock => 'Blocks';

  @override
  String get showMarkdown => 'Show markdown';

  @override
  String get hideMarkdown => 'Hide markdown';
}
