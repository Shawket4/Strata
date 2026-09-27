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
  String homeEdited({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat('d MMM', localeName);
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String get homeSecondaryLabel => 'Today, inbox and AI activity';

  @override
  String get homeAiActivity => 'AI activity';

  @override
  String get homeAiActivityUnavailable =>
      'Relations the AI adds, contradictions it finds and automatic changes you can undo will show here.';

  @override
  String get homeOpenItems => 'Open items';

  @override
  String get homeOpenItemsUnavailable =>
      'Open items from people and company pages will be rolled up here.';

  @override
  String get homeNotYetAvailable => 'Not available yet';
}
