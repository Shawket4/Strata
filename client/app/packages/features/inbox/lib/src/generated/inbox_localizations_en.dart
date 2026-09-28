// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'inbox_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class InboxLocalizationsEn extends InboxLocalizations {
  InboxLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get inboxTitle => 'Inbox';

  @override
  String get inboxLoadError => 'Couldn\'t load the inbox';

  @override
  String get inboxEmptyTitle => 'Inbox zero';

  @override
  String get inboxEmptyMessage =>
      'Captures you save land here, and the AI proposes where they go. Nothing is filed until you accept.';

  @override
  String get inboxAccept => 'Accept';

  @override
  String get inboxReject => 'Reject';

  @override
  String get inboxEdit => 'Edit';

  @override
  String get inboxCancel => 'Cancel';

  @override
  String get inboxUndo => 'Undo';

  @override
  String get inboxDiscard => 'Discard';

  @override
  String get inboxDismiss => 'Dismiss';

  @override
  String get inboxActionFailed => 'Couldn\'t apply that change';

  @override
  String inboxActionFailedCode({required String code}) {
    return 'Couldn\'t apply that change ($code)';
  }

  @override
  String inboxAiConfidence({required String score}) {
    return 'AI · $score';
  }

  @override
  String get inboxAiProposal => 'AI PROPOSAL';

  @override
  String get inboxNoProposalYet =>
      'No proposal yet — the AI has not read this capture.';

  @override
  String inboxProposalCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count proposals',
      one: '1 proposal',
    );
    return '$_temp0';
  }

  @override
  String inboxTag({required String tag}) {
    return '#$tag';
  }

  @override
  String get inboxPeopleAndCompanies => 'PEOPLE & COMPANIES';

  @override
  String inboxCreatesNoteIn({required String folder}) {
    return 'Accepting files it as a new note in $folder';
  }

  @override
  String get inboxRejectRelation => 'Reject relation';

  @override
  String get inboxTaskProposal => 'Proposed task';

  @override
  String get inboxNeedsYou => 'Needs you';

  @override
  String get inboxAppliedAutomatically => 'Applied automatically';

  @override
  String get inboxCustodySuggestion => 'Needs you · custody';

  @override
  String get inboxPossibleDuplicate => 'Possible duplicate';

  @override
  String get inboxRelationSuggestion => 'Relation suggestion';

  @override
  String get inboxTaskSuggestion => 'Task suggestion';

  @override
  String get inboxFilingProposal => 'Filing proposal';

  @override
  String get inboxUnsupportedBadge => 'Suggestion';

  @override
  String inboxUnsupported({required String kind}) {
    return 'This app version can’t show “$kind” suggestions yet. Update the app to review it.';
  }

  @override
  String get inboxRejected => 'Rejected';

  @override
  String inboxWhoIs({required String mention}) {
    return 'Who is “$mention”?';
  }

  @override
  String get inboxNoPersonMatches =>
      'A nickname. No person in your vault matches it yet.';

  @override
  String get inboxPossibleMatches => 'Possible matches in your vault:';

  @override
  String get inboxCreatePerson => 'Create person…';

  @override
  String inboxAliasNote({required String mention}) {
    return 'Accepting adds “$mention” as an alias, so future mentions resolve.';
  }

  @override
  String get inboxCreatePersonTitle => 'Create person';

  @override
  String get inboxPersonName => 'Name';

  @override
  String get inboxAliases => 'ALIASES';

  @override
  String get inboxCreatePersonSave => 'Create';

  @override
  String get inboxWhichDocument => 'Which document?';

  @override
  String get inboxCaptureSemantics => 'Capture';

  @override
  String get inboxBulkActions => 'Bulk actions';

  @override
  String get inboxSelectAll => 'Select all captures';

  @override
  String inboxSelectedCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selected',
      one: '1 selected',
      zero: 'None selected',
    );
    return '$_temp0';
  }

  @override
  String get inboxClearSelection => 'Clear selection';

  @override
  String get inboxSelectCapture => 'Select capture';

  @override
  String get inboxSuggestionsHeader => 'Suggestions';

  @override
  String get inboxCapturesHeader => 'Captures';

  @override
  String get inboxAlsoNeedsYou => 'Also needs you';

  @override
  String get inboxCaptureHeader => 'CAPTURE';

  @override
  String inboxRelationSummary({required String type, required String target}) {
    return '$type $target';
  }

  @override
  String get inboxShortcutsLabel => 'Keyboard shortcuts';

  @override
  String get inboxKeyMove => 'move';

  @override
  String get inboxKeySelect => 'select';

  @override
  String get inboxKeyAccept => 'accept';

  @override
  String get inboxKeyReject => 'reject';

  @override
  String get inboxKeyEdit => 'edit';

  @override
  String get inboxReady => 'Ready';

  @override
  String get inboxLooksRight => 'Looks right';

  @override
  String get inboxKeepBoth => 'Keep both';

  @override
  String inboxYouSaid({required String words}) {
    return 'You said: “$words”';
  }

  @override
  String get inboxNickname => 'Nickname';

  @override
  String inboxItIs({required String title}) {
    return 'It\'s $title';
  }

  @override
  String get inboxCreateCompany => 'Create company…';

  @override
  String inboxQuote({required String quote}) {
    return '“$quote”';
  }

  @override
  String inboxAfterAt({required String place}) {
    return 'Then at $place';
  }

  @override
  String inboxAfterWith({required String person}) {
    return 'Then with $person';
  }

  @override
  String inboxAfterLastWith({required String person}) {
    return 'Nobody has it · last with $person';
  }

  @override
  String inboxThreadAi({required String when}) {
    return 'AI · $when';
  }

  @override
  String inboxThreadYou({required String when}) {
    return 'You · $when';
  }

  @override
  String get inboxReply => 'Reply to the AI';

  @override
  String get inboxReplyField => 'Your reply';

  @override
  String get inboxReplySend => 'Send reply';

  @override
  String get inboxEditTitle => 'Edit before accepting';

  @override
  String get inboxEditTaskText => 'Task';

  @override
  String get inboxEditNoteTitle => 'Note title';

  @override
  String get inboxEditFolder => 'Folder';

  @override
  String get inboxAcceptEdited => 'Accept with changes';

  @override
  String inboxFilterAll({required int count}) {
    return 'All · $count';
  }

  @override
  String inboxFilterNeedsYou({required int count}) {
    return 'Needs you · $count';
  }

  @override
  String inboxFilterConflicts({required int count}) {
    return 'Conflicts · $count';
  }

  @override
  String get inboxFilterLabel => 'Show';

  @override
  String inboxAcceptAllReady({required int count}) {
    return 'Accept all ready · $count';
  }
}
