// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'ask_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AskLocalizationsEn extends AskLocalizations {
  AskLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get askTitle => 'Ask';

  @override
  String get conversation => 'Conversation';

  @override
  String get newConversation => 'New conversation';

  @override
  String get offlineTitle => 'Ask needs a connection';

  @override
  String get offlineMessage =>
      'Your notes are still searchable offline. Ask works again when you are back online.';

  @override
  String get notYetTitle => 'Ask isn\'t available on this server yet';

  @override
  String get notYetMessage =>
      'Answers with citations will appear here once the server supports Ask.';

  @override
  String get notAllowedTitle => 'Ask isn\'t available for this account';

  @override
  String get emptyTitle => 'Ask about your notes';

  @override
  String get emptyMessage =>
      'Answers cite the notes they come from, in English or Arabic.';

  @override
  String get you => 'You';

  @override
  String get answer => 'Answer';

  @override
  String sources({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sources · $count',
      one: 'Sources · 1',
      zero: 'No sources',
    );
    return '$_temp0';
  }

  @override
  String get saveAsNote => 'Save as note';

  @override
  String get copyAnswer => 'Copy answer';

  @override
  String get copied => 'Answer copied';

  @override
  String get question => 'Question';

  @override
  String get askHint => 'Ask about your notes — English or العربية';

  @override
  String get askAboutNoteHint => 'Ask about this note';

  @override
  String get leaveNoteThread => 'Ask about all notes';

  @override
  String get send => 'Send';

  @override
  String get scope => 'Scope';

  @override
  String get sourcePreview => 'Source preview';

  @override
  String get sourcePreviewHint => 'Select a citation to preview its note';

  @override
  String get openAtBlock => 'Open at block';

  @override
  String get noteMissing => 'The cited note isn\'t on this device';

  @override
  String get searchTitle => 'Search';

  @override
  String get searchField => 'Search your notes';

  @override
  String get searchHint => 'Search in English or العربية';

  @override
  String get searchMode => 'Search mode';

  @override
  String get modeKeyword => 'Keyword';

  @override
  String get modeSemantic => 'Semantic';

  @override
  String get modeHybrid => 'Hybrid';

  @override
  String get searchPrompt => 'Type to search';

  @override
  String get searchPromptMessage =>
      'Keyword search works offline; semantic and hybrid search need the server.';

  @override
  String noResults({required String query}) {
    return 'No results for “$query”';
  }

  @override
  String get noResultsMessage => 'Try other words, or the other script.';

  @override
  String modeOffline({required String mode}) {
    return '$mode search needs a connection — showing nothing until you are online.';
  }

  @override
  String modeNotYet({required String mode}) {
    return '$mode search isn\'t available on this server yet.';
  }

  @override
  String modeNotAllowed({required String mode}) {
    return '$mode search isn\'t available for this account.';
  }

  @override
  String resultsCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count results',
      one: '1 result',
    );
    return '$_temp0';
  }

  @override
  String get openNote => 'Open note';

  @override
  String get preview => 'Preview';

  @override
  String get previewHint => 'Select a result to preview it';

  @override
  String get loading => 'Loading…';

  @override
  String get errorTitle => 'This couldn\'t be loaded';

  @override
  String errorMessage({required String code}) {
    return 'The app\'s local data returned an error ($code).';
  }

  @override
  String get stop => 'Stop';

  @override
  String get answering => 'Answering…';

  @override
  String get answerStopped => 'You stopped this answer.';

  @override
  String get answerPaused => 'AI is paused — this answer stopped early.';

  @override
  String get answerUnavailable =>
      'AI isn\'t reachable — this answer stopped early.';

  @override
  String get answerFailed => 'This answer stopped early.';

  @override
  String get savedAsNote => 'Saved as a note';

  @override
  String get openSavedNote => 'Saved · Open note';

  @override
  String citationMarker({required int index}) {
    return 'Citation $index';
  }

  @override
  String score({required String value}) {
    return 'Score $value';
  }
}
