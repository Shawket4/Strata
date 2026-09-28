// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get splashLoading => 'Opening Strata';

  @override
  String get bootFailedTitle => 'Strata couldn\'t start';

  @override
  String bootFailedBody({required String code}) {
    return 'The local database could not be opened ($code).';
  }

  @override
  String get retry => 'Retry';

  @override
  String get notificationDone => 'Done';

  @override
  String get notificationSnooze => 'Snooze';

  @override
  String get notificationOpen => 'Open';

  @override
  String get channelName => 'Reminders';

  @override
  String get channelDescription => 'Task reminders from Strata';

  @override
  String folderSemantics({required String name, required int count}) {
    return '$name, $count notes';
  }

  @override
  String get accountLabel => 'Account';

  @override
  String get foldersTitle => 'Folders';

  @override
  String get pinnedNotesTitle => 'Pinned';

  @override
  String pinnedSemantics({required String title}) {
    return 'Pinned note: $title';
  }
}
