import 'package:flutter/widgets.dart';
import 'package:strata_ui/src/tokens/metrics.dart';

/// Window size classes (PLAN §11, SCREEN_SPEC "Size classes"). Each class has
/// its own navigation and pane structure; layouts are adaptive, never a
/// stretched phone layout.
enum SizeClass {
  /// Width below 600: bottom navigation bar, one pane.
  compact,

  /// Width 600–1199: navigation rail, list + detail, context as a drawer.
  medium,

  /// Width 1200 and above: sidebar, list + main + context panel.
  expanded;

  /// The size class of a window [width] in logical pixels.
  static SizeClass fromWidth(double width) {
    if (width < StrataLayout.mediumBreakpoint) return SizeClass.compact;
    if (width < StrataLayout.expandedBreakpoint) return SizeClass.medium;
    return SizeClass.expanded;
  }

  /// The size class of the window around [context]; rebuilds the caller when
  /// the window width changes (live resize re-evaluates the layout).
  static SizeClass of(BuildContext context) =>
      fromWidth(MediaQuery.sizeOf(context).width);
}
