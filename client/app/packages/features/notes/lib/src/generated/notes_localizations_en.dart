// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'notes_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class NotesLocalizationsEn extends NotesLocalizations {
  NotesLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get notesRoot => 'Notes';

  @override
  String get folderBreadcrumbLabel => 'Folder';

  @override
  String folderNoteCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count notes',
      one: '1 note',
    );
    return '$_temp0';
  }

  @override
  String folderRowSemantics({required String name, required String count}) {
    return 'Folder $name, $count';
  }

  @override
  String get searchNotesLabel => 'Search notes';

  @override
  String get clearSearch => 'Clear search';

  @override
  String notesInFolderLabel({required String folder}) {
    return 'Notes in $folder';
  }

  @override
  String get emptyFolderTitle => 'No notes here yet';

  @override
  String get emptyFolderMessage =>
      'Captures filed into this folder and notes you create here show up in this list.';

  @override
  String noSearchResults({required String query}) {
    return 'No notes match “$query”';
  }

  @override
  String get searchUnavailable => 'Search isn\'t available right now.';

  @override
  String get listErrorTitle => 'Couldn\'t load this folder';

  @override
  String get listErrorMessage =>
      'Something went wrong while reading notes on this device.';

  @override
  String get notSyncedYet => 'Not synced yet';

  @override
  String get selectNoteTitle => 'Select a note';

  @override
  String get selectNoteMessage =>
      'Pick a note from the list to read and edit it.';

  @override
  String get noteViewsLabel => 'Note views';

  @override
  String get tabNote => 'Note';

  @override
  String get tabLinks => 'Links';

  @override
  String get tabHistory => 'History';

  @override
  String get tabBacklinks => 'Backlinks';

  @override
  String get tabGraph => 'Graph';

  @override
  String get contextLabel => 'Note context';

  @override
  String get contextTitle => 'Context';

  @override
  String get showContext => 'Show context panel';

  @override
  String get hideContext => 'Hide context panel';

  @override
  String get openLocalMap => 'Open local mind map';

  @override
  String get moreActions => 'More actions';

  @override
  String get backToNotes => 'Back to notes';

  @override
  String get propertiesTitle => 'Properties';

  @override
  String propertiesSummary({required int relations}) {
    String _temp0 = intl.Intl.pluralLogic(
      relations,
      locale: localeName,
      other: '$relations relations',
      one: '1 relation',
      zero: 'No relations',
    );
    return '$_temp0';
  }

  @override
  String get expandProperties => 'Expand properties';

  @override
  String get collapseProperties => 'Collapse properties';

  @override
  String get relationsLabel => 'Relations';

  @override
  String get tagsLabel => 'Tags';

  @override
  String tagChip({required String tag}) {
    return '⁨#$tag⁩';
  }

  @override
  String get aiSuggestedBy => 'Suggested by AI';

  @override
  String aiConfidence({required String value}) {
    return 'confidence $value';
  }

  @override
  String relationLine({required String type, required String title}) {
    return '$type · $title';
  }

  @override
  String get noReason => 'No reason recorded.';

  @override
  String get relationReject => 'Reject';

  @override
  String get relationRetype => 'Retype';

  @override
  String get retypeTitle => 'Change relation type';

  @override
  String get relationActionsHint => 'Long-press for reject or retype';

  @override
  String get relationUnresolved => 'Not in this vault yet';

  @override
  String get backlinksByType => 'Backlinks by relation type';

  @override
  String get noBacklinks => 'No notes link here yet.';

  @override
  String get backlinkKindLink => 'body links';

  @override
  String get localGraphTitle => 'Local graph';

  @override
  String get openMap => 'Open map';

  @override
  String get localGraphPlaceholder => 'The mind map of this note opens in Map.';

  @override
  String get historyTitle => 'History';

  @override
  String get historyAvailable => 'Versions of this note appear here.';

  @override
  String get historyOffline =>
      'History needs a connection. It\'s back when you\'re online.';

  @override
  String get historyNotYetAvailable => 'History isn\'t available yet.';

  @override
  String get historyNotAllowed => 'History isn\'t available for this account.';

  @override
  String get deleteNote => 'Delete note';

  @override
  String deleteNoteTitle({required String title}) {
    return 'Delete “$title”?';
  }

  @override
  String get deleteNoteMessage => 'The note moves to the vault\'s trash.';

  @override
  String get shortcutsSearch => 'Search notes';
}
