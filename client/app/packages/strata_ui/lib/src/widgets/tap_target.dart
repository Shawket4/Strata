import 'package:flutter/material.dart';
import 'package:strata_ui/src/tokens/metrics.dart';

/// Makes a compact visual (chip, pill) interactive with a hit area of at
/// least 48 × 48 on touch platforms (32 with a pointer), one semantics node
/// carrying [semanticLabel], and keyboard focus.
///
/// When [onTap] and [onLongPress] are both `null` the visual is returned as a
/// labelled, non-interactive semantics node.
class StrataTapTarget extends StatelessWidget {
  /// Creates the tap target.
  const new({
    super.key,
    required this.child,
    required this.semanticLabel,
    this.onTap,
    this.onLongPress,
    this.selected,
  });

  /// The visual.
  final Widget child;

  /// The label announced for the whole target.
  final String semanticLabel;

  /// Called on tap / activation.
  final VoidCallback? onTap;

  /// Called on long press (e.g. the AI relation sheet on phones).
  final VoidCallback? onLongPress;

  /// Selection state, if the target is selectable.
  final bool? selected;

  @override
  Widget build(BuildContext context) {
    final interactive = onTap != null || onLongPress != null;
    if (!interactive) {
      return Semantics(
        container: true,
        label: semanticLabel,
        selected: selected,
        excludeSemantics: true,
        child: child,
      );
    }
    final min = StrataLayout.minTapTarget(context);
    return Semantics(
      container: true,
      button: true,
      label: semanticLabel,
      selected: selected,
      onTap: onTap,
      onLongPress: onLongPress,
      excludeSemantics: true,
      child: Material(
        type: MaterialType.transparency,
        child: InkWell(
          onTap: onTap,
          onLongPress: onLongPress,
          customBorder: const StadiumBorder(),
          child: ConstrainedBox(
            constraints: BoxConstraints(minWidth: min, minHeight: min),
            child: Center(widthFactor: 1, heightFactor: 1, child: child),
          ),
        ),
      ),
    );
  }
}
