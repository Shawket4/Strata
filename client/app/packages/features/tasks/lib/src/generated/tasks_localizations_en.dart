// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'tasks_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class TasksLocalizationsEn extends TasksLocalizations {
  TasksLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get commonCancel => 'Cancel';

  @override
  String get commonSave => 'Save';

  @override
  String get commonLoading => 'Loading';

  @override
  String commonCoreFailure({required String code}) {
    return 'The app reported: $code';
  }

  @override
  String get commonUnknownFailure => 'Something went wrong on this device.';

  @override
  String get commonIntentFailed => 'Couldn\'t apply that change';

  @override
  String commonIntentFailedCode({required String code}) {
    return 'Couldn\'t apply that change ($code)';
  }

  @override
  String get commonNone => '—';

  @override
  String get commonNotSynced => 'Not synced yet';

  @override
  String get commonNotYetAvailable => 'Not available yet';

  @override
  String get dupEyebrow => 'ALREADY EXISTS';

  @override
  String get dupTitle => 'Looks like you already have this';

  @override
  String dupSubtitle({required String kind, required String title}) {
    return 'A similar $kind already covers “$title”.';
  }

  @override
  String get dupOpenExisting => 'Open existing';

  @override
  String get dupCreateAnyway => 'Create anyway';

  @override
  String get dupCancel => 'Cancel';

  @override
  String get dupFootnote =>
      'Create anyway keeps both and won\'t ask about this pair again.';

  @override
  String dupMatch({required String level, required String score}) {
    return '$level · match $score';
  }

  @override
  String get dupMatchExact => 'exact';

  @override
  String get dupMatchNear => 'near';

  @override
  String get dupMatchSemantic => 'semantic';

  @override
  String dupCandidateSemantics({
    required String title,
    required String kind,
    required String match,
  }) {
    return 'Open existing: $title, $kind, $match';
  }

  @override
  String get dupLoadError => 'Couldn\'t load the duplicate check';

  @override
  String get dupNoneOpen => 'Nothing is waiting for a duplicate check.';

  @override
  String get kindTask => 'task';

  @override
  String get kindNote => 'note';

  @override
  String get kindPerson => 'person';

  @override
  String get kindCompany => 'company';

  @override
  String get kindConcept => 'concept';

  @override
  String get kindDocument => 'document';

  @override
  String get kindPlace => 'place';

  @override
  String get kindItem => 'item';

  @override
  String get editorNewTaskTitle => 'New task';

  @override
  String get editorTextLabel => 'What needs doing?';

  @override
  String get editorTextHint =>
      'e.g. Send the weekly invoicing proposal to Ahmed';

  @override
  String get editorParsedTitle => 'UNDERSTOOD AS';

  @override
  String get editorDue => 'Due';

  @override
  String get editorPickDate => 'Pick a date';

  @override
  String get editorRepeat => 'Repeat (rule as written)';

  @override
  String get editorReminders => 'Reminders';

  @override
  String get editorAddReminder => 'Add reminder';

  @override
  String editorReminderAt({required DateTime at}) {
    final intl.DateFormat atDateFormat = intl.DateFormat(
      'EEE d MMM HH:mm',
      localeName,
    );
    final String atString = atDateFormat.format(at);

    return '$atString';
  }

  @override
  String editorRemoveReminder({required String time}) {
    return 'Remove reminder $time';
  }

  @override
  String get editorHomeNote => 'Home note';

  @override
  String get editorDefaultHome => 'Default task list';

  @override
  String get editorClose => 'Close';

  @override
  String get editorSaveFailed => 'Couldn\'t save the task';

  @override
  String get recurrenceTitle => 'Recurrence';

  @override
  String get recurrenceClose => 'Close recurrence editor';

  @override
  String get recurrencePhraseLabel => 'Rule as written';

  @override
  String get recurrencePhraseHint => 'every month on the 1st';

  @override
  String get recurrencePhraseHelper =>
      'Kept verbatim in the note, in the Obsidian Tasks language.';

  @override
  String get recurrenceFrequency => 'FREQUENCY';

  @override
  String get recurrenceDaily => 'Daily';

  @override
  String get recurrenceWeekly => 'Weekly';

  @override
  String get recurrenceMonthly => 'Monthly';

  @override
  String get recurrenceYearly => 'Yearly';

  @override
  String get recurrenceOnDayOfMonth => 'On a day of the month';

  @override
  String get recurrenceOnNthWeekday => 'On the nth weekday';

  @override
  String get recurrenceOnLastDay => 'On the last day';

  @override
  String get recurrencePreviewTitle => 'NEXT DATES';

  @override
  String get recurrenceSave => 'Save rule';

  @override
  String get recurrenceStop => 'Stop repeating';

  @override
  String get tasksTitle => 'Tasks';

  @override
  String get tasksNewTask => 'New task';

  @override
  String get tasksTabToday => 'Today';

  @override
  String get tasksTabUpcoming => 'Upcoming';

  @override
  String get tasksTabOverdue => 'Overdue';

  @override
  String get tasksTabRecurring => 'Recurring';

  @override
  String get tasksTabNoDate => 'No date';

  @override
  String get tasksTabDone => 'Done';

  @override
  String get tasksTabOpen => 'Open';

  @override
  String tasksTabWithCount({required String label, required int count}) {
    return '$label · $count';
  }

  @override
  String get tasksEmptyTitle => 'Nothing here';

  @override
  String get tasksEmptyMessage =>
      'Tasks are checklist lines in your notes. Add one with New task.';

  @override
  String get tasksLoadError => 'Couldn\'t load tasks';

  @override
  String get tasksDetailLoadError => 'Couldn\'t load this task';

  @override
  String get tasksDetailTitle => 'Task';

  @override
  String get tasksDetailPanelLabel => 'Task detail';

  @override
  String get tasksSelectPrompt => 'Select a task to see its details';

  @override
  String get tasksNotFoundTitle => 'Task not found';

  @override
  String get tasksNotFoundMessage =>
      'It may have been deleted or moved on another device.';

  @override
  String tasksDueOn({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'Due $dateString';
  }

  @override
  String tasksOverdueSince({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'Overdue · due $dateString';
  }

  @override
  String tasksDateShort({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String tasksDateLong({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String tasksDoneOn({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat('d MMM', localeName);
    final String dateString = dateDateFormat.format(date);

    return 'Done $dateString';
  }

  @override
  String tasksHistoryDue({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'd MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'due $dateString';
  }

  @override
  String tasksRuleNext({required String rule, required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$rule · next $dateString';
  }

  @override
  String get tasksCancelled => 'Cancelled';

  @override
  String get tasksStateDone => 'Done';

  @override
  String get tasksRecurrenceNotUnderstood => 'Recurrence not understood';

  @override
  String tasksReminderSemantics({required String time}) {
    return 'Reminder $time';
  }

  @override
  String tasksMarkDoneSemantics({required String title}) {
    return 'Mark done: $title';
  }

  @override
  String tasksReopenSemantics({required String title}) {
    return 'Reopen: $title';
  }

  @override
  String get tasksActionMarkDone => 'Mark done';

  @override
  String get tasksActionReopen => 'Reopen';

  @override
  String get tasksActionCancelTask => 'Cancel task';

  @override
  String get tasksActionEditRule => 'Edit rule';

  @override
  String get tasksActionChangeDue => 'Change due date';

  @override
  String get tasksActionClearDue => 'Remove due date';

  @override
  String get tasksActionAddReminder => 'Add reminder';

  @override
  String get tasksActionOpenInNote => 'Open in note';

  @override
  String get tasksActionEditText => 'Edit text';

  @override
  String get tasksActionCloseDetail => 'Close detail';

  @override
  String get tasksRecurring => 'Recurring';

  @override
  String get tasksOneOff => 'One-off';

  @override
  String get tasksFieldRepeat => 'Repeat';

  @override
  String get tasksFieldDue => 'Due';

  @override
  String get tasksFieldReminders => 'Reminders';

  @override
  String get tasksFieldLinked => 'Linked';

  @override
  String get tasksFieldHomeNote => 'Home note';

  @override
  String get tasksFieldStoredLine => 'Stored line';

  @override
  String get tasksDoesNotRepeat => 'Does not repeat';

  @override
  String get tasksNoDueDate => 'No due date';

  @override
  String get tasksNoReminders => 'No reminders';

  @override
  String get tasksNoLinks => 'Nothing linked';

  @override
  String get tasksHistoryTitle => 'History';

  @override
  String get tasksHistorySubtitle => 'completed occurrences';

  @override
  String get tasksHistoryEmpty => 'No completed occurrences yet';

  @override
  String get tasksNoSkipping =>
      'Occurrences can\'t be skipped: an overdue one stays open until it\'s done or cancelled.';

  @override
  String get tasksShortcutsLabel => 'Keyboard shortcuts';

  @override
  String get tasksShortcutMove => 'move';

  @override
  String get tasksShortcutDone => 'done';

  @override
  String get tasksShortcutRepeat => 'repeat';

  @override
  String get tasksShortcutDue => 'due date';

  @override
  String get tasksShortcutOpen => 'open in note';

  @override
  String get tasksColumnTask => 'TASK';

  @override
  String get tasksColumnLinked => 'LINKED';

  @override
  String get tasksColumnRepeat => 'REPEAT';

  @override
  String get tasksColumnRemind => 'REMIND';

  @override
  String get tasksColumnDue => 'DUE';

  @override
  String get tasksFooterNote =>
      'Tasks are Obsidian Tasks lines collected from your notes. Editing here rewrites the line in its home note.';

  @override
  String dupSubtitleUntitled({required String kind}) {
    return 'A similar $kind already exists.';
  }

  @override
  String tasksDueLabel({required String due}) {
    return 'Due $due';
  }

  @override
  String tasksOverdueLabel({required String late, required String due}) {
    return 'Overdue · $late · due $due';
  }

  @override
  String tasksRuleNextLabel({required String rule, required String next}) {
    return '$rule · next $next';
  }

  @override
  String tasksSummary({
    required int open,
    required String done,
    required int notes,
  }) {
    String _temp0 = intl.Intl.pluralLogic(
      open,
      locale: localeName,
      other: '$open open',
      one: '1 open',
      zero: 'Nothing open',
    );
    String _temp1 = intl.Intl.pluralLogic(
      notes,
      locale: localeName,
      other: 'in $notes notes',
      one: 'in 1 note',
      zero: 'in no notes',
    );
    return '$_temp0 · $done · $_temp1';
  }

  @override
  String tasksSummaryOpen({required int open, required int notes}) {
    String _temp0 = intl.Intl.pluralLogic(
      open,
      locale: localeName,
      other: '$open open',
      one: '1 open',
      zero: 'Nothing open',
    );
    String _temp1 = intl.Intl.pluralLogic(
      notes,
      locale: localeName,
      other: 'in $notes notes',
      one: 'in 1 note',
      zero: 'in no notes',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String get recurrencePreviewNone =>
      'Set a due date and a rule the app understands to see the next dates.';

  @override
  String recurrenceInterval({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Every $count periods',
      one: 'Every period',
    );
    return '$_temp0';
  }

  @override
  String get recurrenceIntervalLess => 'Less often';

  @override
  String get recurrenceIntervalMore => 'More apart';

  @override
  String get recurrenceOnDueDay => 'On the due date\'s day';

  @override
  String recurrenceNth({required String nth}) {
    String _temp0 = intl.Intl.selectLogic(nth, {
      '1': 'First',
      '2': 'Second',
      '3': 'Third',
      '4': 'Fourth',
      'other': 'Last',
    });
    return '$_temp0';
  }

  @override
  String get recurrenceWhenDone => 'Count from when it\'s done';

  @override
  String get recurrenceNotUnderstoodHint =>
      'Type a rule, or pick a frequency to build one.';

  @override
  String get weekdayMon => 'Mon';

  @override
  String get weekdayTue => 'Tue';

  @override
  String get weekdayWed => 'Wed';

  @override
  String get weekdayThu => 'Thu';

  @override
  String get weekdayFri => 'Fri';

  @override
  String get weekdaySat => 'Sat';

  @override
  String get weekdaySun => 'Sun';

  @override
  String get editorParsedNothing =>
      'Dates, repeats, reminders and @people in the text show here.';

  @override
  String editorHomeItem({required String title, required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count open',
      one: '1 open',
      zero: 'no open tasks',
    );
    return '$title · $_temp0';
  }
}
