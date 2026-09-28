// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'home_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class HomeLocalizationsEn extends HomeLocalizations {
  HomeLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get homeTitle => 'Home';

  @override
  String get homeLoadError => 'Couldn\'t load Home';

  @override
  String get homeComposerLabel => 'Capture a thought';

  @override
  String get homeComposerHint => 'Type or dictate · Arabic or English';

  @override
  String get homeComposerFooter =>
      'AI files it into your inbox; nothing is filed until you accept.';

  @override
  String get homeComposerOffline =>
      'Offline · captures are kept on this device and sync later.';

  @override
  String get homeFocusHint => 'capture';

  @override
  String get homeSave => 'Save';

  @override
  String get homeCaptureSaved => 'Saved to Inbox';

  @override
  String get homeCaptureFailed => 'Couldn\'t save the capture';

  @override
  String homeCaptureFailedCode({required String code}) {
    return 'Couldn\'t save the capture ($code)';
  }

  @override
  String homeInboxWaiting({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count captures waiting for filing',
      one: '1 capture waiting for filing',
      zero: 'Inbox is clear',
    );
    return '$_temp0';
  }

  @override
  String get homeAllTasks => 'All tasks';

  @override
  String get homeAllNotes => 'All notes';

  @override
  String get homeNoOpenTasks =>
      'No open tasks. Add one from a capture or with New task.';

  @override
  String get homeNothingToday => 'Nothing due today.';

  @override
  String get homeRecentNotes => 'Recent notes';

  @override
  String get homeNoNotes =>
      'No notes yet. Your first capture becomes one once you accept it.';

  @override
  String get homeColumnNote => 'NOTE';

  @override
  String get homeColumnPath => 'PATH';

  @override
  String get homeColumnEdited => 'EDITED';

  @override
  String get homeSecondaryLabel => 'Today, inbox and AI activity';

  @override
  String get homeAiActivity => 'AI activity';

  @override
  String get homeAiActivityUnavailable =>
      'Relations the AI adds, contradictions it finds and automatic changes you can undo show here when the server is reachable.';

  @override
  String get homeOpenItems => 'Open items';

  @override
  String get homeOpenItemsUnavailable =>
      'Open items from people and company pages will be rolled up here.';

  @override
  String get homeNotYetAvailable => 'Not available yet';

  @override
  String homeNeedsYou({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count need you',
      one: '1 needs you',
    );
    return '$_temp0';
  }

  @override
  String homeContradictions({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count contradictions',
      one: '1 contradiction',
    );
    return '$_temp0';
  }

  @override
  String homeLinks({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count links',
      one: '1 link',
    );
    return '$_temp0';
  }

  @override
  String get homeRecentFilter => 'Show';

  @override
  String get homeRecentEdited => 'Edited';

  @override
  String get homeRecentCreated => 'Created';

  @override
  String get homeRecentFiledByAi => 'Filed by AI';

  @override
  String homeActionFailed({required String code}) {
    return 'That didn\'t work ($code).';
  }

  @override
  String get homeOffline => 'Offline';

  @override
  String get homeNotAllowed => 'Not available for this account';

  @override
  String get homeRetypeTitle => 'Change relation type';

  @override
  String homeAiConfidence({required String value}) {
    return 'AI · $value';
  }

  @override
  String get homeAiUndone => 'Undone';

  @override
  String get homeAiRetype => 'Change type';

  @override
  String get homeAiUndo => 'Undo';

  @override
  String get homeAiActivityEmpty => 'Nothing new from the AI.';

  @override
  String get homeOpenItemsEmpty => 'No open items.';
}
