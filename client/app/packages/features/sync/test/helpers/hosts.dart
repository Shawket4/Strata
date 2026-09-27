import 'package:flutter/material.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// A screen with one button that opens the sync status in the surface of
/// the window's size class; records conflicts opened from it.
class SyncHost extends StatelessWidget {
  /// Creates the host.
  const new({super.key, this.opened});

  /// Receives the op IDs of conflicts opened from the surface.
  final List<String>? opened;

  /// The open button's label.
  static const String openLabel = 'Open sync';

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Center(
      child: Builder(
        builder: (context) => FilledButton(
          onPressed: () => showSyncStatus(context, onOpenConflict: opened?.add),
          child: const Text(openLabel),
        ),
      ),
    ),
  );
}

/// The sync status surface of [sizeClass] drawn in place (for goldens,
/// which cannot open routes).
class SurfacePreview extends StatelessWidget {
  /// Creates the preview.
  const new({required this.sizeClass, super.key});

  /// Which surface.
  final SizeClass sizeClass;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Scaffold(
      body: Stack(
        children: [
          Positioned.fill(child: ColoredBox(color: colors.background)),
          switch (sizeClass) {
            SizeClass.compact => Positioned.fill(
              child: ColoredBox(
                color: colors.scrim,
                child: Align(
                  alignment: Alignment.bottomCenter,
                  child: Material(
                    color: colors.surface,
                    borderRadius: StrataRadii.sheetTopRadius,
                    child: const FractionallySizedBox(
                      heightFactor: 0.72,
                      child: SyncStatusPanel(surface: SyncSurface.sheet),
                    ),
                  ),
                ),
              ),
            ),
            SizeClass.medium => Positioned.fill(
              child: ColoredBox(
                color: colors.scrim,
                child: const Align(
                  alignment: AlignmentDirectional.centerEnd,
                  child: SyncDrawer(),
                ),
              ),
            ),
            SizeClass.expanded => const Align(
              alignment: AlignmentDirectional.bottomStart,
              child: Padding(
                padding: EdgeInsetsDirectional.only(start: 12, bottom: 72),
                child: SyncPopover(),
              ),
            ),
          },
        ],
      ),
    );
  }
}
