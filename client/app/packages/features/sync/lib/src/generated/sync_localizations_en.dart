// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'sync_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class SyncLocalizationsEn extends SyncLocalizations {
  SyncLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String pillSemantics({required String status}) {
    return 'Sync status: $status';
  }

  @override
  String get pillOpenHint => 'Open sync status';

  @override
  String get bodyOnline => 'Changes sync automatically with your server.';

  @override
  String get bodyOffline =>
      'Your changes are saved on this device and will sync when you\'re back online.';

  @override
  String get bodyUnknown => 'Waiting for the first answer from your server.';

  @override
  String get phaseIdle => 'Idle';

  @override
  String get phaseBootstrapping => 'Downloading your vault';

  @override
  String get phasePushing => 'Sending changes';

  @override
  String get phasePulling => 'Pulling changes';

  @override
  String get phaseBackoff => 'Waiting to retry';

  @override
  String progressPages({required int done, required int total}) {
    return '$done of $total pages';
  }

  @override
  String progressPagesUnknown({required int done}) {
    return '$done pages';
  }

  @override
  String progressOps({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes in flight',
      one: '1 change in flight',
      zero: 'No changes in flight',
    );
    return '$_temp0';
  }

  @override
  String retryAt({required String time}) {
    return 'Next attempt at $time';
  }

  @override
  String get lastSynced => 'Last synced';

  @override
  String get lastSyncedNever => 'Never';

  @override
  String get server => 'Server';

  @override
  String get snapshot => 'Full copy on this device';

  @override
  String get snapshotDone => 'Yes';

  @override
  String get snapshotPending => 'Not yet';

  @override
  String outboxTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes queued',
      one: '1 change queued',
      zero: 'Nothing queued',
    );
    return '$_temp0';
  }

  @override
  String get outboxOrder => 'Oldest first';

  @override
  String get outboxEmpty => 'Every change on this device has synced.';

  @override
  String get statusPending => 'Queued';

  @override
  String get statusInflight => 'Sending';

  @override
  String get statusConflict => 'Conflict';

  @override
  String get statusDuplicate => 'Already exists?';

  @override
  String attempts({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count attempts',
      one: '1 attempt',
      zero: '',
    );
    return '$_temp0';
  }

  @override
  String get kindNoteCreate => 'New note';

  @override
  String get kindNoteUpdate => 'Edit note';

  @override
  String get kindNoteMove => 'Move note';

  @override
  String get kindNoteDelete => 'Delete note';

  @override
  String get kindEntityCreate => 'New person or company';

  @override
  String get kindEntityPatch => 'Edit person or company';

  @override
  String get kindEntityMerge => 'Merge';

  @override
  String get kindDocument => 'Document change';

  @override
  String get kindPlace => 'Place change';

  @override
  String get kindRelationAdd => 'Add relation';

  @override
  String get kindRelationRemove => 'Remove relation';

  @override
  String get kindRelationRetype => 'Change relation type';

  @override
  String get kindRelink => 'Relink request';

  @override
  String get kindSuggestionAccept => 'Accept suggestion';

  @override
  String get kindSuggestionReject => 'Reject suggestion';

  @override
  String get kindSuggestionReply => 'Answer suggestion';

  @override
  String get kindTaskCreate => 'New task';

  @override
  String get kindTaskUpdate => 'Edit task';

  @override
  String get kindTaskComplete => 'Complete task';

  @override
  String get kindTaskCancel => 'Cancel task';

  @override
  String get kindTaskReopen => 'Reopen task';

  @override
  String get kindTaskDelete => 'Delete task';

  @override
  String get kindDeviceSettings => 'Device settings';

  @override
  String get kindOther => 'Change';

  @override
  String conflictsTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count conflicts need review',
      one: '1 conflict needs review',
    );
    return '$_temp0';
  }

  @override
  String get conflictRow => 'Edited here and on the server';

  @override
  String get review => 'Review';

  @override
  String reviewSemantics({required String title}) {
    return 'Review conflict on $title';
  }

  @override
  String get rejectionsTitle => 'Changes the server refused';

  @override
  String get dismiss => 'Dismiss';

  @override
  String dismissSemantics({required String message}) {
    return 'Dismiss notice: $message';
  }

  @override
  String get lastError => 'Last error';

  @override
  String get retryNow => 'Retry now';

  @override
  String get syncNow => 'Sync now';

  @override
  String get close => 'Close';

  @override
  String get panelTitle => 'Sync';

  @override
  String get loading => 'Loading sync status';

  @override
  String get loadFailed => 'Sync status could not be loaded';

  @override
  String get conflictBreadcrumb => 'Sync › Conflicts';

  @override
  String get conflictIntro =>
      'This device and the server both edited this note. Changes that don\'t overlap were merged; parts changed on both sides need your choice.';

  @override
  String get conflictCleanIntro =>
      'This device and the server both edited this note. The changes don\'t overlap and merged cleanly.';

  @override
  String get conflictGone => 'This conflict is resolved';

  @override
  String get conflictGoneBody => 'Nothing is left to decide here.';

  @override
  String get columnDevice => 'This device';

  @override
  String get columnMerged => 'Merged result';

  @override
  String get columnMergedEditable => 'editable';

  @override
  String get columnServer => 'Server';

  @override
  String get columnBase => 'Base';

  @override
  String get versionMissing => 'Not available yet';

  @override
  String get mergedFieldLabel => 'Merged note text';

  @override
  String hunkTitle({required String location}) {
    return 'Changed on both sides · $location';
  }

  @override
  String get hunkOurs => 'Keep this device\'s text';

  @override
  String get hunkTheirs => 'Keep the server\'s text';

  @override
  String get hunkBase => 'Keep the original';

  @override
  String get hunkBoth => 'Keep both, this device first';

  @override
  String get hunkOwn => 'Write my own';

  @override
  String hunkOwnField({required String location}) {
    return 'Your text for $location';
  }

  @override
  String hunksLeft({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count choices left',
      one: '1 choice left',
      zero: 'Every choice made',
    );
    return '$_temp0';
  }

  @override
  String get conflictFooter =>
      'Nothing is lost — other versions stay in History.';

  @override
  String get keepServer => 'Keep server';

  @override
  String get keepDevice => 'Keep this device';

  @override
  String get keepMerged => 'Keep merged';

  @override
  String get decideLater => 'Close and decide later';

  @override
  String resolveFailed({required String message}) {
    return 'The conflict could not be resolved: $message';
  }

  @override
  String get errorOffline => 'You\'re offline.';

  @override
  String get errorNotFound => 'It no longer exists.';

  @override
  String errorGeneric({required String code}) {
    return 'Something went wrong ($code).';
  }

  @override
  String pillSemanticsWithHint({required String status}) {
    return 'Sync status: $status. Open sync status';
  }

  @override
  String get bodySyncing => 'Sending your changes, then pulling the server\'s.';

  @override
  String get bodyConflict =>
      'A note was edited here and on the server. Review it to keep syncing it.';

  @override
  String get bodyDuplicates =>
      'A change looks like something that already exists. Choose what to keep.';

  @override
  String get bodyPaused =>
      'Sync is paused. Your changes stay on this device until you resume.';

  @override
  String get bodyError => 'The last sync failed. It retries automatically.';

  @override
  String progressItems({required int done, required int total}) {
    return '$done of $total items';
  }

  @override
  String pulledCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count changes pulled',
      one: '1 change pulled',
      zero: 'Nothing pulled yet',
    );
    return '$_temp0';
  }

  @override
  String get pauseSync => 'Pause sync';

  @override
  String get resumeSync => 'Resume sync';

  @override
  String get syncLog => 'Sync log';

  @override
  String get syncLogEmpty => 'Nothing logged yet.';

  @override
  String get saveBothCopies => 'Save both as copies';

  @override
  String conflictCopySaved({required String path}) {
    return 'A copy was saved at $path';
  }

  @override
  String get legendAdded => 'Added';

  @override
  String get legendRemoved => 'Removed';

  @override
  String get legendChanged => 'Changed on both sides';

  @override
  String lineSemantics({
    required int number,
    required String change,
    required String text,
  }) {
    return 'Line $number, $change: $text';
  }

  @override
  String get lineSame => 'unchanged';

  @override
  String get lineChangedOneSide => 'changed';

  @override
  String get hunkBothServerFirst => 'Keep both, server first';
}
