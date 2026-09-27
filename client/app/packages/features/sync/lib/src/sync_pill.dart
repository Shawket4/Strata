import 'package:flutter/material.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_sync/src/labels.dart';
import 'package:strata_ui/strata_ui.dart';

/// The always-visible sync indicator, rendered 1:1 from the core's
/// [SyncPill]: connectivity (tone, icon, copy), queued ops, an activity
/// spinner while the engine runs and a conflict badge.
///
/// [dense] is the icon-only form for the navigation rail (the label stays in
/// the tooltip and semantics).
class SyncStatusPill extends StatelessWidget {
  /// Creates the pill for [pill].
  const new({required this.pill, super.key, this.onPressed, this.dense = false});

  /// The core's pill view-model.
  final SyncPill pill;

  /// Opens the sync status (sheet, drawer or popover).
  final VoidCallback? onPressed;

  /// Icon-only form.
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final label = SyncLabels.pill(l10n, pill);
    final conflicts = pill.conflicts > 0
        ? l10n.pillConflicts(count: pill.conflicts)
        : null;
    final full = conflicts == null
        ? label
        : l10n.pillWithConflicts(status: label, conflicts: conflicts);
    final tone = conflicts == null
        ? SyncLabels.tone(pill.connectivity)
        : StatusTone.danger;
    final icon = SyncLabels.icon(pill.connectivity);
    final running = pill.activity.phase != SyncPhase.idle;
    final Widget visual;
    if (dense) {
      final palette = tone.colorsIn(context.strataColors);
      visual = Tooltip(
        message: full,
        excludeFromSemantics: true,
        child: Container(
          width: 36,
          height: 36,
          decoration: BoxDecoration(
            color: palette.background,
            shape: BoxShape.circle,
          ),
          child: Icon(
            running ? Icons.sync : icon,
            size: 18,
            color: palette.foreground,
          ),
        ),
      );
    } else {
      visual = ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: StrataLayout.sidebarWidth),
        child: StatusPill(
          label: full,
          tone: tone,
          icon: icon,
          trailing: running
              ? SizedBox.square(
                  dimension: 10,
                  child: CircularProgressIndicator(
                    strokeWidth: 1.5,
                    color: tone.colorsIn(context.strataColors).foreground,
                  ),
                )
              : null,
        ),
      );
    }
    return StrataTapTarget(
      semanticLabel: onPressed == null
          ? l10n.pillSemantics(status: full)
          : l10n.pillSemanticsWithHint(status: full),
      onTap: onPressed,
      child: visual,
    );
  }
}
