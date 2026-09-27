// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'tasks_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class TasksLocalizationsAr extends TasksLocalizations {
  TasksLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String tasksDue({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'الاستحقاق $dateString';
  }
}
