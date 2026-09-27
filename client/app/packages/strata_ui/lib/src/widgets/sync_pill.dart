import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/widgets/status_pill.dart';
import 'package:strata_ui/src/widgets/tap_target.dart';

/// Sync state rendered by [SyncPill]. The Rust core streams the state; the
/// widget only maps it to copy, tone and icon.
sealed class SyncStatus {
  const new();
}

/// Everything is synced.
final class SyncSynced extends SyncStatus {
  /// Creates the state with the formatted time of the last sync.
  const new({required this.lastSync});

  /// Time of the last sync as formatted by the core, e.g. `14:32`.
  final String lastSync;
}

/// Offline with queued outbox operations.
final class SyncOffline extends SyncStatus {
  /// Creates the state.
  const new({required this.queued});

  /// Pending outbox operations.
  final int queued;
}

/// Sync in progress.
final class SyncInProgress extends SyncStatus {
  /// Creates the state.
  const new({required this.done, required this.total});

  /// Operations done.
  final int done;

  /// Operations in this sync.
  final int total;
}

/// Unresolved conflicts.
final class SyncConflict extends SyncStatus {
  /// Creates the state.
  const new({required this.count});

  /// Number of conflicts.
  final int count;
}

/// The always-visible sync indicator: "Synced · 14:32", "Offline · 3 changes
/// queued", "Syncing 12/40", "1 conflict". [dense] renders an icon-only pill
/// for the navigation rail (label as tooltip and semantics).
class SyncPill extends StatelessWidget {
  /// Creates the sync pill.
  const new({
    super.key,
    required this.status,
    this.onPressed,
    this.dense = false,
  });

  /// The sync state.
  final SyncStatus status;

  /// Opens the sync status sheet / drawer / popover.
  final VoidCallback? onPressed;

  /// Icon-only form.
  final bool dense;

  /// The localized label of [status].
  static String labelOf(StrataLocalizations l10n, SyncStatus status) =>
      switch (status) {
        SyncSynced(:final lastSync) => l10n.syncSynced(time: lastSync),
        SyncOffline(:final queued) => l10n.syncOfflineQueued(count: queued),
        SyncInProgress(:final done, :final total) => l10n.syncSyncing(
          done: done,
          total: total,
        ),
        SyncConflict(:final count) => l10n.syncConflicts(count: count),
      };

  /// The tone of [status].
  static StatusTone toneOf(SyncStatus status) => switch (status) {
    SyncSynced() => StatusTone.success,
    SyncOffline() => StatusTone.warning,
    SyncInProgress() => StatusTone.info,
    SyncConflict() => StatusTone.danger,
  };

  /// The icon of [status].
  static IconData iconOf(SyncStatus status) => switch (status) {
    SyncSynced() => Icons.cloud_done_outlined,
    SyncOffline() => Icons.cloud_off_outlined,
    SyncInProgress() => Icons.sync,
    SyncConflict() => Icons.report_problem_outlined,
  };

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final label = labelOf(l10n, status);
    final tone = toneOf(status);
    final icon = iconOf(status);
    final Widget visual;
    if (dense) {
      final palette = tone.colorsIn(context.strataColors);
      visual = Tooltip(
        message: label,
        excludeFromSemantics: true,
        child: Container(
          width: 32,
          height: 32,
          decoration: BoxDecoration(
            color: palette.background,
            shape: BoxShape.circle,
          ),
          child: Icon(icon, size: 18, color: palette.foreground),
        ),
      );
    } else {
      visual = ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: StrataLayout.sidebarWidth),
        child: StatusPill(label: label, tone: tone, icon: icon),
      );
    }
    return StrataTapTarget(
      semanticLabel: onPressed == null
          ? label
          : '$label, ${l10n.syncStatusLabel}',
      onTap: onPressed,
      child: visual,
    );
  }
}
