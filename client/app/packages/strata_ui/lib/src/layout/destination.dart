import 'package:flutter/widgets.dart';

/// Where a destination appears in the navigation.
enum DestinationPlacement {
  /// In the main list (bottom bar when [StrataDestination.showInCompact], rail
  /// and sidebar always).
  primary,

  /// At the bottom of the rail and sidebar (e.g. Settings); never in the
  /// compact bottom bar.
  footer,
}

/// A navigation destination passed to the adaptive scaffold. `strata_ui` has
/// no knowledge of features: the app shell supplies every destination.
@immutable
class StrataDestination {
  /// Creates a destination.
  const new({
    required this.icon,
    required this.label,
    this.selectedIcon,
    this.count,
    this.showInCompact = true,
    this.placement = DestinationPlacement.primary,
    this.compactHostIndex,
  });

  /// Icon when not selected.
  final IconData icon;

  /// Icon when selected; defaults to [icon].
  final IconData? selectedIcon;

  /// Visible (and semantic) label.
  final String label;

  /// Optional count shown in the sidebar (e.g. inbox items).
  final int? count;

  /// Whether the destination is part of the compact bottom bar (at most five
  /// destinations may be).
  final bool showInCompact;

  /// Main list or footer.
  final DestinationPlacement placement;

  /// On compact, the index (in the full destination list) of the destination
  /// that hosts this one when it is not in the bottom bar (e.g. Tasks live on
  /// Home). Defaults to the first compact destination.
  final int? compactHostIndex;

  /// The label announced by assistive technology, including the count.
  String get semanticLabel => count == null ? label : '$label, $count';
}

/// A labelled action such as "New capture".
@immutable
class StrataAction {
  /// Creates an action.
  const new({
    required this.label,
    required this.icon,
    required this.onPressed,
    this.shortcutKeys,
  });

  /// Visible and semantic label.
  final String label;

  /// Icon.
  final IconData icon;

  /// Called when activated.
  final VoidCallback onPressed;

  /// Keyboard hint shown on desktop layouts, e.g. `['⌘', 'N']`.
  final List<String>? shortcutKeys;
}
