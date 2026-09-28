import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_sync/src/labels.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Where the sync status is shown; only density and chrome differ.
enum SyncSurface {
  /// Compact bottom sheet.
  sheet,

  /// Medium context drawer.
  drawer,

  /// Expanded sidebar popover.
  popover,

  /// Full screen (`/sync`).
  page,
}

/// The sync status (§11 screen 12b): connectivity, engine activity, last
/// sync, the outbox in push order, conflicts to review and rejected ops,
/// streamed from `syncStatusProvider`.
class SyncStatusPanel extends ConsumerWidget {
  /// Creates the panel.
  const new({
    required this.surface,
    super.key,
    this.onOpenConflict,
    this.onClose,
  });

  /// The surface hosting the panel.
  final SyncSurface surface;

  /// Opens the conflict screen for an op ID.
  final ValueChanged<String>? onOpenConflict;

  /// Closes the hosting sheet, drawer or popover.
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.syncL10n;
    final status = ref.watch(syncStatusProvider);
    return switch (status) {
      AsyncData(:final value) => SyncStatusContent(
        view: value,
        surface: surface,
        onOpenConflict: onOpenConflict,
        onClose: onClose,
        onSyncNow: () => ref.read(coreApiProvider).syncNow(),
        onDismissRejection: (opId) =>
            ref.read(coreApiProvider).dismissRejection(opId: opId),
        onPausedChanged: (paused) =>
            ref.read(coreApiProvider).setSyncPaused(paused: paused),
      ),
      AsyncError(:final error) => StrataEmptyState(
        icon: Icons.sync_problem_outlined,
        title: l10n.loadFailed,
        message: SyncLabels.failure(l10n, error),
      ),
      _ => Center(
        child: Semantics(
          label: l10n.loading,
          child: const CircularProgressIndicator(),
        ),
      ),
    };
  }
}

/// The panel's content for one [SyncStatusView] (no providers; used by
/// [SyncStatusPanel] and directly in tests and goldens).
class SyncStatusContent extends StatelessWidget {
  /// Creates the content.
  const new({
    required this.view,
    required this.surface,
    required this.onSyncNow,
    required this.onDismissRejection,
    super.key,
    this.onOpenConflict,
    this.onClose,
    this.onPausedChanged,
  });

  /// The core's sync status.
  final SyncStatusView view;

  /// Hosting surface.
  final SyncSurface surface;

  /// "Sync now" / "Retry now".
  final VoidCallback onSyncNow;

  /// Dismisses a rejected op's notice.
  final ValueChanged<String> onDismissRejection;

  /// Opens a conflict.
  final ValueChanged<String>? onOpenConflict;

  /// Closes the surface.
  final VoidCallback? onClose;

  /// "Pause sync" / "Resume sync" (the new paused state).
  final ValueChanged<bool>? onPausedChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final pill = view.pill;
    final retry = view.retryLabel;
    final pause = onPausedChanged;
    final pad = surface == SyncSurface.popover
        ? StrataSpacing.s4
        : StrataSpacing.s5;
    final lastError = view.lastError;
    final close = onClose;
    return SingleChildScrollView(
      padding: EdgeInsets.fromLTRB(pad, StrataSpacing.s2, pad, pad),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _ToneBadge(display: pill.display),
              const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Semantics(
                      header: true,
                      container: true,
                      child: Text(
                        pill.label,
                        style: surface == SyncSurface.popover
                            ? text.titleSmall
                            : text.title,
                      ),
                    ),
                    const SizedBox(height: StrataSpacing.s1),
                    Text(
                      SyncLabels.body(l10n, pill.display),
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  ],
                ),
              ),
              if (close != null)
                IconButton(
                  tooltip: l10n.close,
                  onPressed: close,
                  icon: const Icon(Icons.close),
                ),
            ],
          ),
          if (pill.activity.phase != SyncPhase.idle) ...[
            const SizedBox(height: StrataSpacing.s4),
            _Activity(activity: pill.activity, retryLabel: retry),
          ],
          const SizedBox(height: StrataSpacing.s4),
          _Details(view: view),
          if (view.conflicts.isNotEmpty) ...[
            const SizedBox(height: StrataSpacing.s4),
            _Conflicts(conflicts: view.conflicts, onOpen: onOpenConflict),
          ],
          const SizedBox(height: StrataSpacing.s4),
          _Outbox(items: view.outbox),
          if (view.rejections.isNotEmpty) ...[
            const SizedBox(height: StrataSpacing.s4),
            _Rejections(items: view.rejections, onDismiss: onDismissRejection),
          ],
          if (lastError != null) ...[
            const SizedBox(height: StrataSpacing.s4),
            Text(
              l10n.lastError,
              style: text.caption.copyWith(color: colors.text2),
            ),
            Text(
              lastError,
              style: text.monoSmall.copyWith(color: colors.dangerText),
            ),
          ],
          const SizedBox(height: StrataSpacing.s5),
          FilledButton.icon(
            onPressed: onSyncNow,
            style: FilledButton.styleFrom(
              minimumSize: const Size.fromHeight(StrataLayout.minTouchTarget),
            ),
            icon: const Icon(Icons.sync, size: 18),
            label: Text(
              pill.connectivity == Connectivity.offline
                  ? l10n.retryNow
                  : l10n.syncNow,
            ),
          ),
          if (retry != null && pill.activity.phase != SyncPhase.backoff) ...[
            const SizedBox(height: StrataSpacing.s2),
            Text(
              retry,
              textAlign: TextAlign.center,
              style: text.caption.copyWith(color: colors.text2),
            ),
          ],
          if (pause != null) ...[
            const SizedBox(height: StrataSpacing.s2),
            OutlinedButton.icon(
              onPressed: () => pause(!view.paused),
              style: OutlinedButton.styleFrom(
                minimumSize: const Size.fromHeight(StrataLayout.minTouchTarget),
              ),
              icon: Icon(
                view.paused ? Icons.play_arrow_outlined : Icons.pause,
                size: 18,
              ),
              label: Text(view.paused ? l10n.resumeSync : l10n.pauseSync),
            ),
          ],
          const SizedBox(height: StrataSpacing.s3),
          _Log(items: view.log),
        ],
      ),
    );
  }
}

class _ToneBadge extends StatelessWidget {
  const new({required this.display});

  final SyncPillKind display;

  @override
  Widget build(BuildContext context) {
    final palette = SyncLabels.tone(display).colorsIn(context.strataColors);
    return ExcludeSemantics(
      child: Container(
        width: 44,
        height: 44,
        decoration: BoxDecoration(
          color: palette.background,
          shape: BoxShape.circle,
        ),
        child: Icon(
          SyncLabels.icon(display),
          color: palette.foreground,
          size: 22,
        ),
      ),
    );
  }
}

class _Activity extends StatelessWidget {
  const new({required this.activity, required this.retryLabel});

  final SyncActivity activity;
  final String? retryLabel;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final total = activity.pagesTotal;
    final opsTotal = activity.opsTotal;
    final details = [
      switch (activity.phase) {
        SyncPhase.bootstrapping =>
          total == null
              ? l10n.progressPagesUnknown(done: activity.pagesDone)
              : l10n.progressPages(done: activity.pagesDone, total: total),
        SyncPhase.backoff => retryLabel,
        _ when opsTotal > 0 => l10n.progressItems(
          done: activity.opsDone,
          total: opsTotal,
        ),
        SyncPhase.pushing => l10n.progressOps(count: activity.ops),
        _ => null,
      },
      if (activity.phase == SyncPhase.pulling)
        l10n.pulledCount(count: activity.pulled),
    ];
    return Semantics(
      container: true,
      liveRegion: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(SyncLabels.phase(l10n, activity.phase), style: text.bodyStrong),
          const SizedBox(height: StrataSpacing.s2),
          ClipRRect(
            borderRadius: StrataRadii.pillRadius,
            child: LinearProgressIndicator(
              minHeight: 6,
              // The core's counters, drawn as a fraction.
              value: opsTotal > 0 ? activity.opsDone / opsTotal : null,
              color: colors.accent,
              backgroundColor: colors.surface2,
            ),
          ),
          for (final detail in details)
            if (detail != null) ...[
              const SizedBox(height: StrataSpacing.s1),
              Text(detail, style: text.caption.copyWith(color: colors.text2)),
            ],
        ],
      ),
    );
  }
}

class _Details extends StatelessWidget {
  const new({required this.view});

  final SyncStatusView view;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    return _Card(
      child: Column(
        children: [
          _DetailRow(
            label: l10n.lastSynced,
            value: view.pill.lastSyncLabel ?? l10n.lastSyncedNever,
          ),
          _DetailRow(
            label: l10n.snapshot,
            value: view.bootstrapComplete
                ? l10n.snapshotDone
                : l10n.snapshotPending,
          ),
        ],
      ),
    );
  }
}

class _DetailRow extends StatelessWidget {
  const new({required this.label, required this.value, this.mono = false});

  final String label;
  final String value;
  final bool mono;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Text(
              label,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          ),
          const SizedBox(width: StrataSpacing.s3),
          Flexible(
            child: Align(
              alignment: AlignmentDirectional.centerEnd,
              child: Text(
                value,
                textAlign: TextAlign.end,
                textDirection: mono ? TextDirection.ltr : null,
                style: (mono ? text.monoSmall : text.bodySmall).copyWith(
                  color: colors.text,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Card extends StatelessWidget {
  const new({required this.child, this.tint, this.border});

  final Widget child;
  final Color? tint;
  final Color? border;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Container(
      padding: const EdgeInsets.all(StrataSpacing.s3),
      decoration: BoxDecoration(
        color: tint ?? colors.surface,
        borderRadius: StrataRadii.cardRadius,
        border: Border.all(color: border ?? colors.border),
      ),
      child: child,
    );
  }
}

class _Outbox extends StatelessWidget {
  const new({required this.items});

  final List<OutboxItem> items;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Semantics(
                header: true,
                container: true,
                child: Text(
                  l10n.outboxTitle(count: items.length),
                  style: text.bodyStrong,
                ),
              ),
            ),
            if (items.isNotEmpty)
              Text(
                l10n.outboxOrder,
                style: text.caption.copyWith(color: colors.text2),
              ),
          ],
        ),
        const SizedBox(height: StrataSpacing.s2),
        if (items.isEmpty)
          Text(
            l10n.outboxEmpty,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else
          _Card(
            child: Column(
              children: [
                for (final (i, item) in items.indexed) ...[
                  if (i > 0) Divider(height: 1, color: colors.border),
                  _OutboxRow(item: item),
                ],
              ],
            ),
          ),
      ],
    );
  }
}

class _OutboxRow extends StatelessWidget {
  const new({required this.item});

  final OutboxItem item;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final title = item.title;
    final detail = item.detail;
    return MergeSemantics(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: colors.surface2,
                borderRadius: StrataRadii.inputRadius,
              ),
              child: Icon(
                SyncLabels.statusIcon(item.status),
                size: 18,
                color: colors.text2,
              ),
            ),
            const SizedBox(width: StrataSpacing.s3),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    SyncLabels.kind(l10n, item.kind),
                    style: text.bodyStrong,
                  ),
                  if (title != null)
                    Text(
                      title,
                      textAlign: TextAlign.start,
                      style: text.bodySmall,
                    ),
                  if (detail.isNotEmpty)
                    Text(
                      detail,
                      textDirection: textDirectionOf(item.detailDir),
                      textAlign: TextAlign.start,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  Text(
                    SyncLabels.status(l10n, item.status),
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                ],
              ),
            ),
            const SizedBox(width: StrataSpacing.s2),
            Text(
              item.createdLabel,
              style: text.caption.copyWith(color: colors.text2),
            ),
          ],
        ),
      ),
    );
  }
}

class _Conflicts extends StatelessWidget {
  const new({required this.conflicts, required this.onOpen});

  final List<ConflictItem> conflicts;
  final ValueChanged<String>? onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = onOpen;
    return _Card(
      tint: colors.dangerTint,
      border: colors.danger.withValues(alpha: 0.35),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(Icons.report_problem_outlined, color: colors.dangerText),
              const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: Semantics(
                  header: true,
                  container: true,
                  child: Text(
                    l10n.conflictsTitle(count: conflicts.length),
                    style: text.bodyStrong.copyWith(color: colors.dangerText),
                  ),
                ),
              ),
            ],
          ),
          for (final conflict in conflicts)
            Padding(
              padding: const EdgeInsets.only(top: StrataSpacing.s2),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(conflict.title, style: text.bodyStrong),
                        Text(
                          l10n.conflictRow,
                          style: text.caption.copyWith(color: colors.text),
                        ),
                        Text(
                          conflict.createdLabel,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                      ],
                    ),
                  ),
                  Semantics(
                    label: l10n.reviewSemantics(title: conflict.title),
                    excludeSemantics: true,
                    button: true,
                    child: OutlinedButton(
                      onPressed: open == null
                          ? null
                          : () => open(conflict.opId),
                      child: Text(l10n.review),
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _Rejections extends StatelessWidget {
  const new({required this.items, required this.onDismiss});

  final List<RejectionItem> items;
  final ValueChanged<String> onDismiss;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return _Card(
      tint: colors.warningTint,
      border: colors.warning.withValues(alpha: 0.35),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Semantics(
            header: true,
            container: true,
            child: Text(
              l10n.rejectionsTitle,
              style: text.bodyStrong.copyWith(color: colors.warningText),
            ),
          ),
          for (final item in items)
            Padding(
              padding: const EdgeInsets.only(top: StrataSpacing.s2),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          SyncLabels.kind(l10n, item.kind),
                          style: text.bodyStrong,
                        ),
                        Text(
                          item.messageKey,
                          textDirection: TextDirection.ltr,
                          style: text.monoSmall.copyWith(color: colors.text),
                        ),
                      ],
                    ),
                  ),
                  Semantics(
                    label: l10n.dismissSemantics(message: item.messageKey),
                    excludeSemantics: true,
                    button: true,
                    child: TextButton(
                      onPressed: () => onDismiss(item.opId),
                      child: Text(l10n.dismiss),
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

/// The sync log (newest first, the core's order), folded by default.
class _Log extends StatelessWidget {
  const new({required this.items});

  final List<SyncLogItem> items;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        tilePadding: EdgeInsets.zero,
        childrenPadding: EdgeInsets.zero,
        title: Text(l10n.syncLog, style: text.bodyStrong),
        children: [
          if (items.isEmpty)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: Text(
                l10n.syncLogEmpty,
                style: text.bodySmall.copyWith(color: colors.text2),
              ),
            ),
          for (final item in items)
            MergeSemantics(
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      item.atLabel,
                      textDirection: TextDirection.ltr,
                      style: text.monoSmall.copyWith(color: colors.text2),
                    ),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(child: Text(item.detail, style: text.bodySmall)),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}
