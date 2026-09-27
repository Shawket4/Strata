import 'package:flutter/material.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_sync/src/sync_panel.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Opens the sync status in the surface of the window's size class: a
/// bottom sheet on compact, a drawer from the end edge on medium and a
/// popover over the sidebar's sync block on expanded (SCREEN_SPEC
/// SyncCompact / SyncMedium / SyncExpanded).
///
/// [onOpenConflict] is called after the surface closed.
Future<void> showSyncStatus(
  BuildContext context, {
  ValueChanged<String>? onOpenConflict,
}) {
  final l10n = context.syncL10n;
  void Function(String)? open(BuildContext surfaceContext) =>
      onOpenConflict == null
      ? null
      : (opId) {
          Navigator.of(surfaceContext).pop();
          onOpenConflict(opId);
        };
  switch (SizeClass.of(context)) {
    case SizeClass.compact:
      return showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        builder: (sheetContext) => FractionallySizedBox(
          heightFactor: 0.72,
          child: SyncStatusPanel(
            surface: SyncSurface.sheet,
            onOpenConflict: open(sheetContext),
            onClose: () => Navigator.of(sheetContext).pop(),
          ),
        ),
      );
    case SizeClass.medium:
      return showGeneralDialog<void>(
        context: context,
        barrierDismissible: true,
        barrierLabel: l10n.close,
        barrierColor: context.strataColors.scrim,
        pageBuilder: (dialogContext, _, _) => Align(
          alignment: AlignmentDirectional.centerEnd,
          child: SyncDrawer(
            onOpenConflict: open(dialogContext),
            onClose: () => Navigator.of(dialogContext).pop(),
          ),
        ),
        transitionBuilder: (context, animation, _, child) {
          final rtl = Directionality.of(context) == TextDirection.rtl;
          return SlideTransition(
            position: Tween(begin: Offset(rtl ? -1 : 1, 0), end: Offset.zero)
                .animate(
                  CurvedAnimation(parent: animation, curve: Curves.easeOut),
                ),
            child: child,
          );
        },
      );
    case SizeClass.expanded:
      return showDialog<void>(
        context: context,
        barrierColor: Colors.transparent,
        builder: (dialogContext) => Align(
          alignment: AlignmentDirectional.bottomStart,
          child: Padding(
            padding: const EdgeInsetsDirectional.only(
              start: StrataSpacing.s3,
              bottom: StrataSpacing.s12 + StrataSpacing.s6,
            ),
            child: SyncPopover(
              onOpenConflict: open(dialogContext),
              onClose: () => Navigator.of(dialogContext).pop(),
            ),
          ),
        ),
      );
  }
}

/// The medium-size sync drawer (380 wide, full height, from the end edge).
class SyncDrawer extends StatelessWidget {
  /// Creates the drawer.
  const new({super.key, this.onOpenConflict, this.onClose});

  /// Opens a conflict.
  final ValueChanged<String>? onOpenConflict;

  /// Closes the drawer.
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Semantics(
      scopesRoute: true,
      namesRoute: true,
      explicitChildNodes: true,
      label: context.syncL10n.panelTitle,
      child: Material(
        color: colors.surface,
        shape: BorderDirectional(start: BorderSide(color: colors.border)),
        child: SizedBox(
          width: 380,
          height: double.infinity,
          child: SafeArea(
            child: SyncStatusPanel(
              surface: SyncSurface.drawer,
              onOpenConflict: onOpenConflict,
              onClose: onClose,
            ),
          ),
        ),
      ),
    );
  }
}

/// The expanded-size sync popover (392 wide) over the sidebar.
class SyncPopover extends StatelessWidget {
  /// Creates the popover.
  const new({super.key, this.onOpenConflict, this.onClose});

  /// Opens a conflict.
  final ValueChanged<String>? onOpenConflict;

  /// Closes the popover.
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Semantics(
      scopesRoute: true,
      namesRoute: true,
      explicitChildNodes: true,
      label: context.syncL10n.panelTitle,
      child: Container(
        width: 392,
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * 0.8,
        ),
        decoration: BoxDecoration(
          color: colors.surface,
          borderRadius: StrataRadii.cardRadius,
          border: Border.all(color: colors.border),
          boxShadow: StrataElevation.popover,
        ),
        child: Material(
          type: MaterialType.transparency,
          child: SyncStatusPanel(
            surface: SyncSurface.popover,
            onOpenConflict: onOpenConflict,
            onClose: onClose,
          ),
        ),
      ),
    );
  }
}

/// The `/sync` screen: the sync status as a page (deep links, and the
/// target of "Review" when no surface is open).
class SyncScreen extends StatelessWidget {
  /// Creates the screen.
  const new({super.key, this.onOpenConflict});

  /// The icon that represents this feature.
  static const IconData icon = Icons.sync;

  /// Opens a conflict.
  final ValueChanged<String>? onOpenConflict;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: AlignmentDirectional.topCenter,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720),
        child: SyncStatusPanel(
          surface: SyncSurface.page,
          onOpenConflict: onOpenConflict,
        ),
      ),
    );
  }
}
