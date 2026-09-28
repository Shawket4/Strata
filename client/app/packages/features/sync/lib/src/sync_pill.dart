import 'package:flutter/material.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_sync/src/labels.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// The always-visible sync indicator, rendered 1:1 from the core's
/// [SyncPill]: the display state the core picked ([SyncPill.display]: tone
/// and icon), its ready label ("Synced · 14:32", "Offline · 3 queued",
/// "Syncing 12/40", "1 conflict") and a spinner while syncing.
///
/// [dense] is the icon-only form for the navigation rail (the label stays in
/// the tooltip and semantics).
class SyncStatusPill extends StatelessWidget {
  /// Creates the pill for [pill].
  const new({
    required this.pill,
    super.key,
    this.onPressed,
    this.dense = false,
  });

  /// The core's pill view-model.
  final SyncPill pill;

  /// Opens the sync status (sheet, drawer or popover).
  final VoidCallback? onPressed;

  /// Icon-only form.
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final label = pill.label;
    final tone = SyncLabels.tone(pill.display);
    final icon = SyncLabels.icon(pill.display);
    final running = pill.display == SyncPillKind.syncing;
    final palette = tone.colorsIn(context.strataColors);
    final Widget visual;
    if (dense) {
      visual = Tooltip(
        message: label,
        excludeFromSemantics: true,
        child: Container(
          width: 36,
          height: 36,
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
        child: StatusPill(
          label: label,
          tone: tone,
          icon: icon,
          trailing: running
              ? SizedBox.square(
                  dimension: 10,
                  child: CircularProgressIndicator(
                    strokeWidth: 1.5,
                    color: palette.foreground,
                  ),
                )
              : null,
        ),
      );
    }
    return StrataTapTarget(
      semanticLabel: onPressed == null
          ? l10n.pillSemantics(status: label)
          : l10n.pillSemanticsWithHint(status: label),
      onTap: onPressed,
      child: visual,
    );
  }
}
