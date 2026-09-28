import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'sync_localizations_ar.dart';
import 'sync_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of SyncLocalizations
/// returned by `SyncLocalizations.of(context)`.
///
/// Applications need to include `SyncLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/sync_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: SyncLocalizations.localizationsDelegates,
///   supportedLocales: SyncLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the SyncLocalizations.supportedLocales
/// property.
abstract class SyncLocalizations {
  SyncLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static SyncLocalizations of(BuildContext context) {
    return Localizations.of<SyncLocalizations>(context, SyncLocalizations)!;
  }

  static const LocalizationsDelegate<SyncLocalizations> delegate =
      _SyncLocalizationsDelegate();

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

  /// Accessibility label of the sync pill.
  ///
  /// In en, this message translates to:
  /// **'Sync status: {status}'**
  String pillSemantics({required String status});

  /// Accessibility hint: the pill opens the sync status.
  ///
  /// In en, this message translates to:
  /// **'Open sync status'**
  String get pillOpenHint;

  /// Sync panel description online.
  ///
  /// In en, this message translates to:
  /// **'Changes sync automatically with your server.'**
  String get bodyOnline;

  /// Sync panel description offline.
  ///
  /// In en, this message translates to:
  /// **'Your changes are saved on this device and will sync when you\'re back online.'**
  String get bodyOffline;

  /// Sync panel description before the first request.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the first answer from your server.'**
  String get bodyUnknown;

  /// Sync engine phase: nothing running.
  ///
  /// In en, this message translates to:
  /// **'Idle'**
  String get phaseIdle;

  /// Sync engine phase: bootstrap.
  ///
  /// In en, this message translates to:
  /// **'Downloading your vault'**
  String get phaseBootstrapping;

  /// Sync engine phase: push.
  ///
  /// In en, this message translates to:
  /// **'Sending changes'**
  String get phasePushing;

  /// Sync engine phase: pull.
  ///
  /// In en, this message translates to:
  /// **'Pulling changes'**
  String get phasePulling;

  /// Sync engine phase: backoff.
  ///
  /// In en, this message translates to:
  /// **'Waiting to retry'**
  String get phaseBackoff;

  /// Bootstrap progress with a known total.
  ///
  /// In en, this message translates to:
  /// **'{done} of {total} pages'**
  String progressPages({required int done, required int total});

  /// Bootstrap progress without a known total.
  ///
  /// In en, this message translates to:
  /// **'{done} pages'**
  String progressPagesUnknown({required int done});

  /// Ops in the push in flight.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No changes in flight} =1{1 change in flight} other{{count} changes in flight}}'**
  String progressOps({required int count});

  /// When the next sync attempt runs.
  ///
  /// In en, this message translates to:
  /// **'Next attempt at {time}'**
  String retryAt({required String time});

  /// Label of the last sync time.
  ///
  /// In en, this message translates to:
  /// **'Last synced'**
  String get lastSynced;

  /// No sync completed yet.
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get lastSyncedNever;

  /// Label of the server URL.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get server;

  /// Label: the bootstrap finished.
  ///
  /// In en, this message translates to:
  /// **'Full copy on this device'**
  String get snapshot;

  /// The bootstrap is complete.
  ///
  /// In en, this message translates to:
  /// **'Yes'**
  String get snapshotDone;

  /// The bootstrap is not complete.
  ///
  /// In en, this message translates to:
  /// **'Not yet'**
  String get snapshotPending;

  /// Outbox heading with the number of queued ops.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Nothing queued} =1{1 change queued} other{{count} changes queued}}'**
  String outboxTitle({required int count});

  /// The outbox is listed in push order.
  ///
  /// In en, this message translates to:
  /// **'Oldest first'**
  String get outboxOrder;

  /// Empty outbox message.
  ///
  /// In en, this message translates to:
  /// **'Every change on this device has synced.'**
  String get outboxEmpty;

  /// Outbox op status: waiting.
  ///
  /// In en, this message translates to:
  /// **'Queued'**
  String get statusPending;

  /// Outbox op status: being pushed.
  ///
  /// In en, this message translates to:
  /// **'Sending'**
  String get statusInflight;

  /// Outbox op status: conflict.
  ///
  /// In en, this message translates to:
  /// **'Conflict'**
  String get statusConflict;

  /// Outbox op status: duplicate prompt.
  ///
  /// In en, this message translates to:
  /// **'Already exists?'**
  String get statusDuplicate;

  /// Push attempts of an op.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{} =1{1 attempt} other{{count} attempts}}'**
  String attempts({required int count});

  /// Outbox op kind note.create.
  ///
  /// In en, this message translates to:
  /// **'New note'**
  String get kindNoteCreate;

  /// Outbox op kind note.update.
  ///
  /// In en, this message translates to:
  /// **'Edit note'**
  String get kindNoteUpdate;

  /// Outbox op kind note.move.
  ///
  /// In en, this message translates to:
  /// **'Move note'**
  String get kindNoteMove;

  /// Outbox op kind note.delete.
  ///
  /// In en, this message translates to:
  /// **'Delete note'**
  String get kindNoteDelete;

  /// Outbox op kind entity.create.
  ///
  /// In en, this message translates to:
  /// **'New person or company'**
  String get kindEntityCreate;

  /// Outbox op kind entity.patch.
  ///
  /// In en, this message translates to:
  /// **'Edit person or company'**
  String get kindEntityPatch;

  /// Outbox op kind entity.merge.
  ///
  /// In en, this message translates to:
  /// **'Merge'**
  String get kindEntityMerge;

  /// Outbox op kinds document.*.
  ///
  /// In en, this message translates to:
  /// **'Document change'**
  String get kindDocument;

  /// Outbox op kinds place.*.
  ///
  /// In en, this message translates to:
  /// **'Place change'**
  String get kindPlace;

  /// Outbox op kind relation.add.
  ///
  /// In en, this message translates to:
  /// **'Add relation'**
  String get kindRelationAdd;

  /// Outbox op kind relation.remove.
  ///
  /// In en, this message translates to:
  /// **'Remove relation'**
  String get kindRelationRemove;

  /// Outbox op kind relation.retype.
  ///
  /// In en, this message translates to:
  /// **'Change relation type'**
  String get kindRelationRetype;

  /// Outbox op kind relink.request.
  ///
  /// In en, this message translates to:
  /// **'Relink request'**
  String get kindRelink;

  /// Outbox op kind suggestion.accept.
  ///
  /// In en, this message translates to:
  /// **'Accept suggestion'**
  String get kindSuggestionAccept;

  /// Outbox op kind suggestion.reject.
  ///
  /// In en, this message translates to:
  /// **'Reject suggestion'**
  String get kindSuggestionReject;

  /// Outbox op kind suggestion.reply.
  ///
  /// In en, this message translates to:
  /// **'Answer suggestion'**
  String get kindSuggestionReply;

  /// Outbox op kind task.create.
  ///
  /// In en, this message translates to:
  /// **'New task'**
  String get kindTaskCreate;

  /// Outbox op kind task.update.
  ///
  /// In en, this message translates to:
  /// **'Edit task'**
  String get kindTaskUpdate;

  /// Outbox op kind task.complete.
  ///
  /// In en, this message translates to:
  /// **'Complete task'**
  String get kindTaskComplete;

  /// Outbox op kind task.cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel task'**
  String get kindTaskCancel;

  /// Outbox op kind task.reopen.
  ///
  /// In en, this message translates to:
  /// **'Reopen task'**
  String get kindTaskReopen;

  /// Outbox op kind task.delete.
  ///
  /// In en, this message translates to:
  /// **'Delete task'**
  String get kindTaskDelete;

  /// Outbox op kind device.settings.
  ///
  /// In en, this message translates to:
  /// **'Device settings'**
  String get kindDeviceSettings;

  /// An op kind this app version does not name.
  ///
  /// In en, this message translates to:
  /// **'Change'**
  String get kindOther;

  /// Conflicts section heading.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 conflict needs review} other{{count} conflicts need review}}'**
  String conflictsTitle({required int count});

  /// Conflict row subtitle.
  ///
  /// In en, this message translates to:
  /// **'Edited here and on the server'**
  String get conflictRow;

  /// Opens a conflict.
  ///
  /// In en, this message translates to:
  /// **'Review'**
  String get review;

  /// Accessibility label of a conflict's Review button.
  ///
  /// In en, this message translates to:
  /// **'Review conflict on {title}'**
  String reviewSemantics({required String title});

  /// Rejected ops section heading.
  ///
  /// In en, this message translates to:
  /// **'Changes the server refused'**
  String get rejectionsTitle;

  /// Dismisses a rejected op notice.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get dismiss;

  /// Accessibility label of Dismiss.
  ///
  /// In en, this message translates to:
  /// **'Dismiss notice: {message}'**
  String dismissSemantics({required String message});

  /// Label of the last sync error.
  ///
  /// In en, this message translates to:
  /// **'Last error'**
  String get lastError;

  /// Sync now while offline or backing off.
  ///
  /// In en, this message translates to:
  /// **'Retry now'**
  String get retryNow;

  /// Sync now.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get syncNow;

  /// Closes the sync panel.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// Title of the sync drawer / screen.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get panelTitle;

  /// Loading state label.
  ///
  /// In en, this message translates to:
  /// **'Loading sync status'**
  String get loading;

  /// Error state title.
  ///
  /// In en, this message translates to:
  /// **'Sync status could not be loaded'**
  String get loadFailed;

  /// Breadcrumb over the conflict screen.
  ///
  /// In en, this message translates to:
  /// **'Sync › Conflicts'**
  String get conflictBreadcrumb;

  /// Conflict screen introduction.
  ///
  /// In en, this message translates to:
  /// **'This device and the server both edited this note. Changes that don\'t overlap were merged; parts changed on both sides need your choice.'**
  String get conflictIntro;

  /// Conflict screen introduction for a clean merge.
  ///
  /// In en, this message translates to:
  /// **'This device and the server both edited this note. The changes don\'t overlap and merged cleanly.'**
  String get conflictCleanIntro;

  /// The conflict no longer exists.
  ///
  /// In en, this message translates to:
  /// **'This conflict is resolved'**
  String get conflictGone;

  /// Body of the resolved-conflict state.
  ///
  /// In en, this message translates to:
  /// **'Nothing is left to decide here.'**
  String get conflictGoneBody;

  /// Local version column.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get columnDevice;

  /// Merged column.
  ///
  /// In en, this message translates to:
  /// **'Merged result'**
  String get columnMerged;

  /// Suffix: the merged result can be edited.
  ///
  /// In en, this message translates to:
  /// **'editable'**
  String get columnMergedEditable;

  /// Server version column.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get columnServer;

  /// Base version.
  ///
  /// In en, this message translates to:
  /// **'Base'**
  String get columnBase;

  /// A side of the conflict is not known yet.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get versionMissing;

  /// Accessibility label of the merged editor.
  ///
  /// In en, this message translates to:
  /// **'Merged note text'**
  String get mergedFieldLabel;

  /// Heading of a conflicting hunk.
  ///
  /// In en, this message translates to:
  /// **'Changed on both sides · {location}'**
  String hunkTitle({required String location});

  /// Hunk choice: ours.
  ///
  /// In en, this message translates to:
  /// **'Keep this device\'s text'**
  String get hunkOurs;

  /// Hunk choice: theirs.
  ///
  /// In en, this message translates to:
  /// **'Keep the server\'s text'**
  String get hunkTheirs;

  /// Hunk choice: base.
  ///
  /// In en, this message translates to:
  /// **'Keep the original'**
  String get hunkBase;

  /// Hunk choice ours then theirs.
  ///
  /// In en, this message translates to:
  /// **'Keep both, this device first'**
  String get hunkBoth;

  /// Hunk choice: own text.
  ///
  /// In en, this message translates to:
  /// **'Write my own'**
  String get hunkOwn;

  /// Label of the own-text field of a hunk.
  ///
  /// In en, this message translates to:
  /// **'Your text for {location}'**
  String hunkOwnField({required String location});

  /// Undecided hunks.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Every choice made} =1{1 choice left} other{{count} choices left}}'**
  String hunksLeft({required int count});

  /// Reassurance in the conflict footer.
  ///
  /// In en, this message translates to:
  /// **'Nothing is lost — other versions stay in History.'**
  String get conflictFooter;

  /// Resolve: keep the server version.
  ///
  /// In en, this message translates to:
  /// **'Keep server'**
  String get keepServer;

  /// Resolve: keep the local version.
  ///
  /// In en, this message translates to:
  /// **'Keep this device'**
  String get keepDevice;

  /// Resolve: keep the merged result.
  ///
  /// In en, this message translates to:
  /// **'Keep merged'**
  String get keepMerged;

  /// Closes the conflict screen.
  ///
  /// In en, this message translates to:
  /// **'Close and decide later'**
  String get decideLater;

  /// Snackbar when resolving fails.
  ///
  /// In en, this message translates to:
  /// **'The conflict could not be resolved: {message}'**
  String resolveFailed({required String message});

  /// Core error offline.
  ///
  /// In en, this message translates to:
  /// **'You\'re offline.'**
  String get errorOffline;

  /// Core error not_found.
  ///
  /// In en, this message translates to:
  /// **'It no longer exists.'**
  String get errorNotFound;

  /// Any other core error.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong ({code}).'**
  String errorGeneric({required String code});

  /// Accessibility label of the sync pill when it opens the sync status.
  ///
  /// In en, this message translates to:
  /// **'Sync status: {status}. Open sync status'**
  String pillSemanticsWithHint({required String status});

  /// Sync panel body while syncing.
  ///
  /// In en, this message translates to:
  /// **'Sending your changes, then pulling the server\'s.'**
  String get bodySyncing;

  /// Sync panel body with a conflict.
  ///
  /// In en, this message translates to:
  /// **'A note was edited here and on the server. Review it to keep syncing it.'**
  String get bodyConflict;

  /// Sync panel body with a duplicate prompt.
  ///
  /// In en, this message translates to:
  /// **'A change looks like something that already exists. Choose what to keep.'**
  String get bodyDuplicates;

  /// Sync panel body while paused.
  ///
  /// In en, this message translates to:
  /// **'Sync is paused. Your changes stay on this device until you resume.'**
  String get bodyPaused;

  /// Sync panel body after an error.
  ///
  /// In en, this message translates to:
  /// **'The last sync failed. It retries automatically.'**
  String get bodyError;

  /// Sync progress.
  ///
  /// In en, this message translates to:
  /// **'{done} of {total} items'**
  String progressItems({required int done, required int total});

  /// Changes pulled so far.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Nothing pulled yet} =1{1 change pulled} other{{count} changes pulled}}'**
  String pulledCount({required int count});

  /// Pause sync button.
  ///
  /// In en, this message translates to:
  /// **'Pause sync'**
  String get pauseSync;

  /// Resume sync button.
  ///
  /// In en, this message translates to:
  /// **'Resume sync'**
  String get resumeSync;

  /// Sync log section.
  ///
  /// In en, this message translates to:
  /// **'Sync log'**
  String get syncLog;

  /// Empty sync log.
  ///
  /// In en, this message translates to:
  /// **'Nothing logged yet.'**
  String get syncLogEmpty;

  /// Conflict: keep the server version and store the local text as a copy.
  ///
  /// In en, this message translates to:
  /// **'Save both as copies'**
  String get saveBothCopies;

  /// Where the conflict copy was saved.
  ///
  /// In en, this message translates to:
  /// **'A copy was saved at {path}'**
  String conflictCopySaved({required String path});

  /// Diff legend: added.
  ///
  /// In en, this message translates to:
  /// **'Added'**
  String get legendAdded;

  /// Diff legend: removed.
  ///
  /// In en, this message translates to:
  /// **'Removed'**
  String get legendRemoved;

  /// Diff legend: changed on both sides.
  ///
  /// In en, this message translates to:
  /// **'Changed on both sides'**
  String get legendChanged;

  /// One annotated line.
  ///
  /// In en, this message translates to:
  /// **'Line {number}, {change}: {text}'**
  String lineSemantics({
    required int number,
    required String change,
    required String text,
  });

  /// Line change: same.
  ///
  /// In en, this message translates to:
  /// **'unchanged'**
  String get lineSame;

  /// Line change: changed on one side.
  ///
  /// In en, this message translates to:
  /// **'changed'**
  String get lineChangedOneSide;

  /// Hunk choice theirs then ours.
  ///
  /// In en, this message translates to:
  /// **'Keep both, server first'**
  String get hunkBothServerFirst;
}

class _SyncLocalizationsDelegate
    extends LocalizationsDelegate<SyncLocalizations> {
  const _SyncLocalizationsDelegate();

  @override
  Future<SyncLocalizations> load(Locale locale) {
    return SynchronousFuture<SyncLocalizations>(
      lookupSyncLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_SyncLocalizationsDelegate old) => false;
}

SyncLocalizations lookupSyncLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return SyncLocalizationsAr();
    case 'en':
      return SyncLocalizationsEn();
  }

  throw FlutterError(
    'SyncLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
