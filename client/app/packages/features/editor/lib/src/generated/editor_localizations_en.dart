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
  String get statusSaved => 'Saved';

  @override
  String get statusUnsaved => 'Unsaved changes';

  @override
  String statusPending({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Saved on this device · $count changes to sync',
      one: 'Saved on this device · 1 change to sync',
    );
    return '$_temp0';
  }

  @override
  String get statusConflict => 'Conflict';

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
  String get suggestionsPeople => 'People';

  @override
  String get suggestionsCompanies => 'Companies';

  @override
  String get suggestionsNotes => 'Notes';

  @override
  String get suggestionsNone => 'No matches';

  @override
  String get suggestionsTypeToSearch => 'Type to search';

  @override
  String get suggestionsTagsUnavailable =>
      'Tag suggestions aren\'t available yet.';

  @override
  String get suggestionsBlocksUnavailable =>
      'Block references can\'t be listed yet.';

  @override
  String get suggestionsSearchUnavailable =>
      'Search isn\'t available right now.';

  @override
  String get suggestionsDismiss => 'Dismiss suggestions';

  @override
  String get statusBarMarkdown => 'Markdown';

  @override
  String get statusBarRtl => 'RTL · auto per paragraph';

  @override
  String get shortcutSave => 'Save';
}
