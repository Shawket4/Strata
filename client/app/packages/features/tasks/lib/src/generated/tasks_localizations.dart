import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'tasks_localizations_ar.dart';
import 'tasks_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of TasksLocalizations
/// returned by `TasksLocalizations.of(context)`.
///
/// Applications need to include `TasksLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/tasks_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: TasksLocalizations.localizationsDelegates,
///   supportedLocales: TasksLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the TasksLocalizations.supportedLocales
/// property.
abstract class TasksLocalizations {
  TasksLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static TasksLocalizations of(BuildContext context) {
    return Localizations.of<TasksLocalizations>(context, TasksLocalizations)!;
  }

  static const LocalizationsDelegate<TasksLocalizations> delegate =
      _TasksLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('en'),
  ];

  /// Common action: cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get commonCancel;

  /// Common action: save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get commonSave;

  /// Semantics label of a progress indicator.
  ///
  /// In en, this message translates to:
  /// **'Loading'**
  String get commonLoading;

  /// Error state message with the core failure code.
  ///
  /// In en, this message translates to:
  /// **'The app reported: {code}'**
  String commonCoreFailure({required String code});

  /// Error state message without a code.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong on this device.'**
  String get commonUnknownFailure;

  /// Snack bar when the core refused an intent.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t apply that change'**
  String get commonIntentFailed;

  /// Snack bar when the core refused an intent, with its failure code.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t apply that change ({code})'**
  String commonIntentFailedCode({required String code});

  /// Empty table cell.
  ///
  /// In en, this message translates to:
  /// **'—'**
  String get commonNone;

  /// Marker for items with unsynced changes.
  ///
  /// In en, this message translates to:
  /// **'Not synced yet'**
  String get commonNotSynced;

  /// Tooltip / state of a feature the core does not provide yet.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get commonNotYetAvailable;

  /// Eyebrow of the duplicate prompt.
  ///
  /// In en, this message translates to:
  /// **'ALREADY EXISTS'**
  String get dupEyebrow;

  /// Duplicate prompt headline.
  ///
  /// In en, this message translates to:
  /// **'Looks like you already have this'**
  String get dupTitle;

  /// Duplicate prompt subtitle: kind and title of the item being created.
  ///
  /// In en, this message translates to:
  /// **'A similar {kind} already covers “{title}”.'**
  String dupSubtitle({required String kind, required String title});

  /// Duplicate prompt action.
  ///
  /// In en, this message translates to:
  /// **'Open existing'**
  String get dupOpenExisting;

  /// Duplicate prompt action.
  ///
  /// In en, this message translates to:
  /// **'Create anyway'**
  String get dupCreateAnyway;

  /// Duplicate prompt action: do not create.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get dupCancel;

  /// Duplicate prompt footnote.
  ///
  /// In en, this message translates to:
  /// **'Create anyway keeps both and won\'t ask about this pair again.'**
  String get dupFootnote;

  /// Match level and score of a duplicate candidate.
  ///
  /// In en, this message translates to:
  /// **'{level} · match {score}'**
  String dupMatch({required String level, required String score});

  /// Duplicate match level.
  ///
  /// In en, this message translates to:
  /// **'exact'**
  String get dupMatchExact;

  /// Duplicate match level.
  ///
  /// In en, this message translates to:
  /// **'near'**
  String get dupMatchNear;

  /// Duplicate match level.
  ///
  /// In en, this message translates to:
  /// **'semantic'**
  String get dupMatchSemantic;

  /// Semantics label of a duplicate candidate card.
  ///
  /// In en, this message translates to:
  /// **'Open existing: {title}, {kind}, {match}'**
  String dupCandidateSemantics({
    required String title,
    required String kind,
    required String match,
  });

  /// Error title of the duplicate prompt.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load the duplicate check'**
  String get dupLoadError;

  /// Duplicate prompt with no open prompts.
  ///
  /// In en, this message translates to:
  /// **'Nothing is waiting for a duplicate check.'**
  String get dupNoneOpen;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'task'**
  String get kindTask;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'note'**
  String get kindNote;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'person'**
  String get kindPerson;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'company'**
  String get kindCompany;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'concept'**
  String get kindConcept;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'document'**
  String get kindDocument;

  /// Item kind.
  ///
  /// In en, this message translates to:
  /// **'place'**
  String get kindPlace;

  /// Item kind (unknown).
  ///
  /// In en, this message translates to:
  /// **'item'**
  String get kindItem;

  /// New task sheet title.
  ///
  /// In en, this message translates to:
  /// **'New task'**
  String get editorNewTaskTitle;

  /// Task text field label.
  ///
  /// In en, this message translates to:
  /// **'What needs doing?'**
  String get editorTextLabel;

  /// Task text field hint.
  ///
  /// In en, this message translates to:
  /// **'e.g. Send the weekly invoicing proposal to Ahmed'**
  String get editorTextHint;

  /// Section title of the parsed task details.
  ///
  /// In en, this message translates to:
  /// **'UNDERSTOOD AS'**
  String get editorParsedTitle;

  /// Parsed details are not provided by the core yet.
  ///
  /// In en, this message translates to:
  /// **'Reading dates and repeats from the text is not available yet'**
  String get editorParsedUnavailable;

  /// Due date field.
  ///
  /// In en, this message translates to:
  /// **'Due'**
  String get editorDue;

  /// Opens the date picker.
  ///
  /// In en, this message translates to:
  /// **'Pick a date'**
  String get editorPickDate;

  /// Recurrence phrase field.
  ///
  /// In en, this message translates to:
  /// **'Repeat (rule as written)'**
  String get editorRepeat;

  /// Reminders field.
  ///
  /// In en, this message translates to:
  /// **'Reminders'**
  String get editorReminders;

  /// Adds a reminder.
  ///
  /// In en, this message translates to:
  /// **'Add reminder'**
  String get editorAddReminder;

  /// A picked reminder date and time.
  ///
  /// In en, this message translates to:
  /// **'{at}'**
  String editorReminderAt({required DateTime at});

  /// Removes a picked reminder.
  ///
  /// In en, this message translates to:
  /// **'Remove reminder {time}'**
  String editorRemoveReminder({required String time});

  /// The note the task line goes into.
  ///
  /// In en, this message translates to:
  /// **'Home note'**
  String get editorHomeNote;

  /// Home note when none is chosen.
  ///
  /// In en, this message translates to:
  /// **'Default task list'**
  String get editorDefaultHome;

  /// Closes the new task sheet.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get editorClose;

  /// Snack bar when creating a task failed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the task'**
  String get editorSaveFailed;

  /// Recurrence editor title.
  ///
  /// In en, this message translates to:
  /// **'Recurrence'**
  String get recurrenceTitle;

  /// Close button of the recurrence editor.
  ///
  /// In en, this message translates to:
  /// **'Close recurrence editor'**
  String get recurrenceClose;

  /// Recurrence phrase field label.
  ///
  /// In en, this message translates to:
  /// **'Rule as written'**
  String get recurrencePhraseLabel;

  /// Recurrence phrase example (Tasks plugin grammar, always English).
  ///
  /// In en, this message translates to:
  /// **'every month on the 1st'**
  String get recurrencePhraseHint;

  /// Recurrence phrase helper text.
  ///
  /// In en, this message translates to:
  /// **'Kept verbatim in the note, in the Obsidian Tasks language.'**
  String get recurrencePhraseHelper;

  /// Recurrence builder section.
  ///
  /// In en, this message translates to:
  /// **'FREQUENCY'**
  String get recurrenceFrequency;

  /// Frequency.
  ///
  /// In en, this message translates to:
  /// **'Daily'**
  String get recurrenceDaily;

  /// Frequency.
  ///
  /// In en, this message translates to:
  /// **'Weekly'**
  String get recurrenceWeekly;

  /// Frequency.
  ///
  /// In en, this message translates to:
  /// **'Monthly'**
  String get recurrenceMonthly;

  /// Frequency.
  ///
  /// In en, this message translates to:
  /// **'Yearly'**
  String get recurrenceYearly;

  /// Monthly option.
  ///
  /// In en, this message translates to:
  /// **'On a day of the month'**
  String get recurrenceOnDayOfMonth;

  /// Monthly option.
  ///
  /// In en, this message translates to:
  /// **'On the nth weekday'**
  String get recurrenceOnNthWeekday;

  /// Monthly option.
  ///
  /// In en, this message translates to:
  /// **'On the last day'**
  String get recurrenceOnLastDay;

  /// Ends option.
  ///
  /// In en, this message translates to:
  /// **'Ends never'**
  String get recurrenceEndsNever;

  /// Ends option.
  ///
  /// In en, this message translates to:
  /// **'Ends on a date'**
  String get recurrenceEndsOnDate;

  /// Ends option.
  ///
  /// In en, this message translates to:
  /// **'Ends after N occurrences'**
  String get recurrenceEndsAfter;

  /// The core rule builder is missing.
  ///
  /// In en, this message translates to:
  /// **'Building a rule from options is not available yet'**
  String get recurrenceBuilderUnavailable;

  /// Preview section title.
  ///
  /// In en, this message translates to:
  /// **'NEXT DATES'**
  String get recurrencePreviewTitle;

  /// The core preview is missing.
  ///
  /// In en, this message translates to:
  /// **'A preview of the next dates isn\'t available yet.'**
  String get recurrencePreviewUnavailable;

  /// Saves the rule.
  ///
  /// In en, this message translates to:
  /// **'Save rule'**
  String get recurrenceSave;

  /// Removes the recurrence.
  ///
  /// In en, this message translates to:
  /// **'Stop repeating'**
  String get recurrenceStop;

  /// Tasks screen title.
  ///
  /// In en, this message translates to:
  /// **'Tasks'**
  String get tasksTitle;

  /// New task action.
  ///
  /// In en, this message translates to:
  /// **'New task'**
  String get tasksNewTask;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get tasksTabToday;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'Upcoming'**
  String get tasksTabUpcoming;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'Overdue'**
  String get tasksTabOverdue;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'Recurring'**
  String get tasksTabRecurring;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'No date'**
  String get tasksTabNoDate;

  /// Task view.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get tasksTabDone;

  /// Task view (expanded table).
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get tasksTabOpen;

  /// A task view with its item count.
  ///
  /// In en, this message translates to:
  /// **'{label} · {count}'**
  String tasksTabWithCount({required String label, required int count});

  /// Empty task list title.
  ///
  /// In en, this message translates to:
  /// **'Nothing here'**
  String get tasksEmptyTitle;

  /// Empty task list message.
  ///
  /// In en, this message translates to:
  /// **'Tasks are checklist lines in your notes. Add one with New task.'**
  String get tasksEmptyMessage;

  /// Tasks error title.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load tasks'**
  String get tasksLoadError;

  /// Task detail error title.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load this task'**
  String get tasksDetailLoadError;

  /// Task detail app bar title.
  ///
  /// In en, this message translates to:
  /// **'Task'**
  String get tasksDetailTitle;

  /// Semantics label of the task detail panel.
  ///
  /// In en, this message translates to:
  /// **'Task detail'**
  String get tasksDetailPanelLabel;

  /// Detail pane without a selection.
  ///
  /// In en, this message translates to:
  /// **'Select a task to see its details'**
  String get tasksSelectPrompt;

  /// Task detail for an unknown id.
  ///
  /// In en, this message translates to:
  /// **'Task not found'**
  String get tasksNotFoundTitle;

  /// Task detail for an unknown id.
  ///
  /// In en, this message translates to:
  /// **'It may have been deleted or moved on another device.'**
  String get tasksNotFoundMessage;

  /// Due date of a task.
  ///
  /// In en, this message translates to:
  /// **'Due {date}'**
  String tasksDueOn({required DateTime date});

  /// Due date of an overdue task.
  ///
  /// In en, this message translates to:
  /// **'Overdue · due {date}'**
  String tasksOverdueSince({required DateTime date});

  /// A short date.
  ///
  /// In en, this message translates to:
  /// **'{date}'**
  String tasksDateShort({required DateTime date});

  /// A long date.
  ///
  /// In en, this message translates to:
  /// **'{date}'**
  String tasksDateLong({required DateTime date});

  /// Done date of a task.
  ///
  /// In en, this message translates to:
  /// **'Done {date}'**
  String tasksDoneOn({required DateTime date});

  /// Due date of a completed occurrence.
  ///
  /// In en, this message translates to:
  /// **'due {date}'**
  String tasksHistoryDue({required DateTime date});

  /// A recurring rule with its next occurrence.
  ///
  /// In en, this message translates to:
  /// **'{rule} · next {date}'**
  String tasksRuleNext({required String rule, required DateTime date});

  /// Cancelled task state.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get tasksCancelled;

  /// Done task state.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get tasksStateDone;

  /// The recurrence phrase is outside the grammar.
  ///
  /// In en, this message translates to:
  /// **'Recurrence not understood'**
  String get tasksRecurrenceNotUnderstood;

  /// Semantics of a reminder.
  ///
  /// In en, this message translates to:
  /// **'Reminder {time}'**
  String tasksReminderSemantics({required String time});

  /// Checkbox label of an open task.
  ///
  /// In en, this message translates to:
  /// **'Mark done: {title}'**
  String tasksMarkDoneSemantics({required String title});

  /// Checkbox label of a closed task.
  ///
  /// In en, this message translates to:
  /// **'Reopen: {title}'**
  String tasksReopenSemantics({required String title});

  /// Task action.
  ///
  /// In en, this message translates to:
  /// **'Mark done'**
  String get tasksActionMarkDone;

  /// Task action.
  ///
  /// In en, this message translates to:
  /// **'Reopen'**
  String get tasksActionReopen;

  /// Task action.
  ///
  /// In en, this message translates to:
  /// **'Cancel task'**
  String get tasksActionCancelTask;

  /// Opens the recurrence editor.
  ///
  /// In en, this message translates to:
  /// **'Edit rule'**
  String get tasksActionEditRule;

  /// Opens the date picker.
  ///
  /// In en, this message translates to:
  /// **'Change due date'**
  String get tasksActionChangeDue;

  /// Removes the due date.
  ///
  /// In en, this message translates to:
  /// **'Remove due date'**
  String get tasksActionClearDue;

  /// Adds a reminder.
  ///
  /// In en, this message translates to:
  /// **'Add reminder'**
  String get tasksActionAddReminder;

  /// Opens the home note.
  ///
  /// In en, this message translates to:
  /// **'Open in note'**
  String get tasksActionOpenInNote;

  /// Edits the task text.
  ///
  /// In en, this message translates to:
  /// **'Edit text'**
  String get tasksActionEditText;

  /// Closes the detail panel.
  ///
  /// In en, this message translates to:
  /// **'Close detail'**
  String get tasksActionCloseDetail;

  /// Recurring task pill.
  ///
  /// In en, this message translates to:
  /// **'Recurring'**
  String get tasksRecurring;

  /// One-off task pill.
  ///
  /// In en, this message translates to:
  /// **'One-off'**
  String get tasksOneOff;

  /// Detail field.
  ///
  /// In en, this message translates to:
  /// **'Repeat'**
  String get tasksFieldRepeat;

  /// Detail field.
  ///
  /// In en, this message translates to:
  /// **'Due'**
  String get tasksFieldDue;

  /// Detail field.
  ///
  /// In en, this message translates to:
  /// **'Reminders'**
  String get tasksFieldReminders;

  /// Detail field.
  ///
  /// In en, this message translates to:
  /// **'Linked'**
  String get tasksFieldLinked;

  /// Detail field.
  ///
  /// In en, this message translates to:
  /// **'Home note'**
  String get tasksFieldHomeNote;

  /// Detail field: the raw task line.
  ///
  /// In en, this message translates to:
  /// **'Stored line'**
  String get tasksFieldStoredLine;

  /// No recurrence.
  ///
  /// In en, this message translates to:
  /// **'Does not repeat'**
  String get tasksDoesNotRepeat;

  /// No due date.
  ///
  /// In en, this message translates to:
  /// **'No due date'**
  String get tasksNoDueDate;

  /// No reminders.
  ///
  /// In en, this message translates to:
  /// **'No reminders'**
  String get tasksNoReminders;

  /// No linked entities.
  ///
  /// In en, this message translates to:
  /// **'Nothing linked'**
  String get tasksNoLinks;

  /// History section.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get tasksHistoryTitle;

  /// History section subtitle.
  ///
  /// In en, this message translates to:
  /// **'completed occurrences'**
  String get tasksHistorySubtitle;

  /// Empty history.
  ///
  /// In en, this message translates to:
  /// **'No completed occurrences yet'**
  String get tasksHistoryEmpty;

  /// Recurring task note.
  ///
  /// In en, this message translates to:
  /// **'Occurrences can\'t be skipped: an overdue one stays open until it\'s done or cancelled.'**
  String get tasksNoSkipping;

  /// Shortcut legend label.
  ///
  /// In en, this message translates to:
  /// **'Keyboard shortcuts'**
  String get tasksShortcutsLabel;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'move'**
  String get tasksShortcutMove;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'done'**
  String get tasksShortcutDone;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'repeat'**
  String get tasksShortcutRepeat;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'due date'**
  String get tasksShortcutDue;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'open in note'**
  String get tasksShortcutOpen;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'TASK'**
  String get tasksColumnTask;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'LINKED'**
  String get tasksColumnLinked;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'REPEAT'**
  String get tasksColumnRepeat;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'REMIND'**
  String get tasksColumnRemind;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'DUE'**
  String get tasksColumnDue;

  /// Footer note of the task table.
  ///
  /// In en, this message translates to:
  /// **'Tasks are Obsidian Tasks lines collected from your notes. Editing here rewrites the line in its home note.'**
  String get tasksFooterNote;

  /// Duplicate prompt subtitle when the title is unknown.
  ///
  /// In en, this message translates to:
  /// **'A similar {kind} already exists.'**
  String dupSubtitleUntitled({required String kind});
}

class _TasksLocalizationsDelegate
    extends LocalizationsDelegate<TasksLocalizations> {
  const _TasksLocalizationsDelegate();

  @override
  Future<TasksLocalizations> load(Locale locale) {
    return SynchronousFuture<TasksLocalizations>(
      lookupTasksLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_TasksLocalizationsDelegate old) => false;
}

TasksLocalizations lookupTasksLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return TasksLocalizationsAr();
    case 'en':
      return TasksLocalizationsEn();
  }

  throw FlutterError(
    'TasksLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
