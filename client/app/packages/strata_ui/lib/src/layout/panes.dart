import 'package:flutter/material.dart';
import 'package:strata_ui/src/layout/size_class.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';

/// List / detail / context pane structure per size class:
///
/// * expanded: list (320) + detail + context panel (340) side by side;
/// * medium: list (320) + detail, context as an overlay drawer from the end
///   edge while [contextPanelOpen];
/// * compact: one pane ([detail] when [showDetailOnCompact], else [list]);
///   context goes into a bottom sheet via [showStrataContextSheet].
class StrataPanes extends StatelessWidget {
  /// Creates the pane layout.
  const new({
    required this.list,
    required this.detail,
    super.key,
    this.contextPanel,
    this.contextPanelOpen = false,
    this.onContextPanelClosed,
    this.contextPanelLabel = 'Context',
    this.dismissLabel = 'Close',
    this.showDetailOnCompact = false,
  });

  /// The list pane.
  final Widget list;

  /// The detail (main) pane.
  final Widget detail;

  /// The context panel (backlinks, relations, graph, history).
  final Widget? contextPanel;

  /// Whether the medium-size overlay drawer is open.
  final bool contextPanelOpen;

  /// Called when the medium-size drawer is dismissed via its scrim.
  final VoidCallback? onContextPanelClosed;

  /// Accessibility label of the context panel.
  final String contextPanelLabel;

  /// Accessibility label of the scrim that closes the drawer.
  final String dismissLabel;

  /// Which pane the compact layout shows.
  final bool showDetailOnCompact;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final divider = VerticalDivider(width: 1, color: colors.border);
    final listPane = SizedBox(width: StrataLayout.listPaneWidth, child: list);
    final panel = contextPanel;
    switch (SizeClass.of(context)) {
      case SizeClass.compact:
        return showDetailOnCompact ? detail : list;
      case SizeClass.medium:
        final row = Row(
          children: [
            listPane,
            divider,
            Expanded(child: detail),
          ],
        );
        if (panel == null || !contextPanelOpen) return row;
        return Stack(
          children: [
            row,
            Positioned.fill(
              child: ModalBarrier(
                color: colors.scrim,
                dismissible: onContextPanelClosed != null,
                onDismiss: onContextPanelClosed,
                semanticsLabel: dismissLabel,
              ),
            ),
            PositionedDirectional(
              top: 0,
              bottom: 0,
              end: 0,
              width: StrataLayout.contextPanelWidth,
              child: _ContextPanel(
                label: contextPanelLabel,
                overlay: true,
                child: panel,
              ),
            ),
          ],
        );
      case SizeClass.expanded:
        return Row(
          children: [
            listPane,
            divider,
            Expanded(child: detail),
            if (panel != null) ...[
              divider,
              SizedBox(
                width: StrataLayout.contextPanelWidth,
                child: _ContextPanel(label: contextPanelLabel, child: panel),
              ),
            ],
          ],
        );
    }
  }
}

class _ContextPanel extends StatelessWidget {
  const new({required this.label, required this.child, this.overlay = false});

  final String label;
  final Widget child;
  final bool overlay;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: label,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.surface,
          boxShadow: overlay ? StrataElevation.popover : null,
        ),
        child: Material(type: MaterialType.transparency, child: child),
      ),
    );
  }
}

/// Shows the context panel content as a modal bottom sheet (compact layout).
Future<T?> showStrataContextSheet<T>({
  required BuildContext context,
  required WidgetBuilder builder,
}) {
  return showModalBottomSheet<T>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: builder,
  );
}
