import 'package:flutter/material.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// 1:1 renderings of the sync view-model's enums and codes (copy, tone,
/// icon). Nothing is derived: every value comes from the core.
abstract final class SyncLabels {
  /// The tone of the pill's display state (picked by the core).
  static StatusTone tone(SyncPillKind display) => switch (display) {
    SyncPillKind.synced => StatusTone.success,
    SyncPillKind.offline || SyncPillKind.paused => StatusTone.warning,
    SyncPillKind.syncing => StatusTone.info,
    SyncPillKind.conflict ||
    SyncPillKind.duplicates ||
    SyncPillKind.error => StatusTone.danger,
  };

  /// The icon of the pill's display state.
  static IconData icon(SyncPillKind display) => switch (display) {
    SyncPillKind.synced => Icons.cloud_done_outlined,
    SyncPillKind.offline => Icons.cloud_off_outlined,
    SyncPillKind.syncing => Icons.sync,
    SyncPillKind.conflict => Icons.report_problem_outlined,
    SyncPillKind.duplicates => Icons.content_copy_outlined,
    SyncPillKind.paused => Icons.pause_circle_outline,
    SyncPillKind.error => Icons.sync_problem_outlined,
  };

  /// The panel description of the pill's display state.
  static String body(SyncLocalizations l10n, SyncPillKind display) =>
      switch (display) {
        SyncPillKind.synced => l10n.bodyOnline,
        SyncPillKind.offline => l10n.bodyOffline,
        SyncPillKind.syncing => l10n.bodySyncing,
        SyncPillKind.conflict => l10n.bodyConflict,
        SyncPillKind.duplicates => l10n.bodyDuplicates,
        SyncPillKind.paused => l10n.bodyPaused,
        SyncPillKind.error => l10n.bodyError,
      };

  /// The name of a sync phase.
  static String phase(SyncLocalizations l10n, SyncPhase phase) =>
      switch (phase) {
        SyncPhase.idle => l10n.phaseIdle,
        SyncPhase.bootstrapping => l10n.phaseBootstrapping,
        SyncPhase.pushing => l10n.phasePushing,
        SyncPhase.pulling => l10n.phasePulling,
        SyncPhase.backoff => l10n.phaseBackoff,
      };

  /// The name of an outbox status.
  static String status(SyncLocalizations l10n, OutboxStatus status) =>
      switch (status) {
        OutboxStatus.pending => l10n.statusPending,
        OutboxStatus.inflight => l10n.statusInflight,
        OutboxStatus.conflict => l10n.statusConflict,
        OutboxStatus.duplicate => l10n.statusDuplicate,
      };

  /// The icon of an outbox status.
  static IconData statusIcon(OutboxStatus status) => switch (status) {
    OutboxStatus.pending => Icons.schedule,
    OutboxStatus.inflight => Icons.upload_outlined,
    OutboxStatus.conflict => Icons.report_problem_outlined,
    OutboxStatus.duplicate => Icons.content_copy_outlined,
  };

  /// The name of an op kind (the wire spelling from `sync-model`).
  static String kind(SyncLocalizations l10n, String kind) => switch (kind) {
    'note.create' => l10n.kindNoteCreate,
    'note.update' => l10n.kindNoteUpdate,
    'note.move' => l10n.kindNoteMove,
    'note.delete' => l10n.kindNoteDelete,
    'entity.create' => l10n.kindEntityCreate,
    'entity.patch' => l10n.kindEntityPatch,
    'entity.merge' => l10n.kindEntityMerge,
    'document.create' ||
    'document.patch' ||
    'document.custody' => l10n.kindDocument,
    'place.create' || 'place.patch' => l10n.kindPlace,
    'relation.add' => l10n.kindRelationAdd,
    'relation.remove' => l10n.kindRelationRemove,
    'relation.retype' => l10n.kindRelationRetype,
    'relink.request' => l10n.kindRelink,
    'suggestion.accept' => l10n.kindSuggestionAccept,
    'suggestion.reject' => l10n.kindSuggestionReject,
    'suggestion.reply' => l10n.kindSuggestionReply,
    'task.create' => l10n.kindTaskCreate,
    'task.update' => l10n.kindTaskUpdate,
    'task.complete' => l10n.kindTaskComplete,
    'task.cancel' => l10n.kindTaskCancel,
    'task.reopen' => l10n.kindTaskReopen,
    'task.delete' => l10n.kindTaskDelete,
    'device.settings' => l10n.kindDeviceSettings,
    _ => l10n.kindOther,
  };

  /// The localised message of a core failure.
  static String failure(SyncLocalizations l10n, Object error) =>
      switch (error) {
        CoreFailure(code: 'offline') => l10n.errorOffline,
        CoreFailure(code: 'not_found') => l10n.errorNotFound,
        CoreFailure(:final code) => l10n.errorGeneric(code: code),
        _ => l10n.errorGeneric(code: 'internal'),
      };
}
