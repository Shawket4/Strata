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
  String get inboxLinkExisting => 'Link to existing person…';

  @override
  String get inboxLinkUnavailable =>
      'Linking to an existing person is not available yet';

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
  String get inboxChoiceUnavailable =>
      'Choosing one of these is not available yet';

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
}
