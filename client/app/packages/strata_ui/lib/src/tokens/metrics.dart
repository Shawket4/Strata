import 'package:flutter/material.dart';

/// Spacing on the 4 px grid (common gaps 8/12/16/24).
abstract final class StrataSpacing {
  /// 4 px.
  static const double s1 = 4;

  /// 8 px.
  static const double s2 = 8;

  /// 12 px.
  static const double s3 = 12;

  /// 16 px.
  static const double s4 = 16;

  /// 20 px.
  static const double s5 = 20;

  /// 24 px.
  static const double s6 = 24;

  /// 32 px.
  static const double s8 = 32;

  /// 40 px.
  static const double s10 = 40;

  /// 48 px.
  static const double s12 = 48;
}

/// Corner radii.
abstract final class StrataRadii {
  /// Inputs and nav items: 8.
  static const double input = 8;

  /// Cards: 12.
  static const double card = 12;

  /// Sheets and popovers: 16.
  static const double sheet = 16;

  /// Chips and pills: fully rounded.
  static const double pill = 999;

  /// [BorderRadius] for inputs.
  static const BorderRadius inputRadius = BorderRadius.all(
    Radius.circular(input),
  );

  /// [BorderRadius] for cards.
  static const BorderRadius cardRadius = BorderRadius.all(
    Radius.circular(card),
  );

  /// [BorderRadius] for pills and chips.
  static const BorderRadius pillRadius = BorderRadius.all(
    Radius.circular(pill),
  );

  /// Top-only [BorderRadius] for bottom sheets.
  static const BorderRadius sheetTopRadius = BorderRadius.vertical(
    top: Radius.circular(sheet),
  );
}

/// Elevation: hairline borders by default; one soft shadow for sheets and
/// popovers only (`0 8px 24px rgba(15,27,38,0.12)`).
abstract final class StrataElevation {
  /// Hairline border width.
  static const double hairline = 1;

  /// The single soft shadow used for sheets, drawers and popovers.
  static const List<BoxShadow> popover = [
    BoxShadow(color: Color(0x1F0F1B26), offset: Offset(0, 8), blurRadius: 24),
  ];
}

/// Layout metrics shared by the adaptive shells and panes.
abstract final class StrataLayout {
  /// Compact → medium breakpoint (logical px).
  static const double mediumBreakpoint = 600;

  /// Medium → expanded breakpoint (logical px).
  static const double expandedBreakpoint = 1200;

  /// Navigation rail width.
  static const double railWidth = 80;

  /// Expanded sidebar width.
  static const double sidebarWidth = 248;

  /// List pane width (medium and expanded).
  static const double listPaneWidth = 320;

  /// Context panel width (expanded pane, medium overlay drawer).
  static const double contextPanelWidth = 340;

  /// Maximum destinations in the compact bottom bar (platform limit).
  static const int maxCompactDestinations = 5;

  /// Minimum tap target on touch platforms.
  static const double minTouchTarget = 48;

  /// Minimum interactive size with a pointer (desktop).
  static const double minPointerTarget = 32;

  /// Whether [platform] is primarily driven by touch.
  static bool isTouch(TargetPlatform platform) => switch (platform) {
    TargetPlatform.android ||
    TargetPlatform.iOS ||
    TargetPlatform.fuchsia => true,
    TargetPlatform.linux ||
    TargetPlatform.macOS ||
    TargetPlatform.windows => false,
  };

  /// The minimum interactive size for the theme's platform.
  static double minTapTarget(BuildContext context) =>
      isTouch(Theme.of(context).platform) ? minTouchTarget : minPointerTarget;
}

/// Motion tokens (durations and curves) as a [ThemeExtension], so reduced
/// motion can swap every duration to zero in one place.
@immutable
class StrataMotion extends ThemeExtension<StrataMotion> {
  /// Creates motion tokens.
  const new({
    required this.short,
    required this.medium,
    required this.long,
    required this.standard,
    required this.emphasized,
    required this.decelerate,
    required this.accelerate,
  });

  /// Default motion.
  static const StrataMotion standardMotion = StrataMotion(
    short: Duration(milliseconds: 120),
    medium: Duration(milliseconds: 200),
    long: Duration(milliseconds: 320),
    standard: Cubic(0.2, 0, 0, 1),
    emphasized: Curves.easeInOutCubicEmphasized,
    decelerate: Cubic(0, 0, 0, 1),
    accelerate: Cubic(0.3, 0, 1, 1),
  );

  /// Reduced motion: same curves, no duration.
  static const StrataMotion reduced = StrataMotion(
    short: Duration.zero,
    medium: Duration.zero,
    long: Duration.zero,
    standard: Cubic(0.2, 0, 0, 1),
    emphasized: Curves.easeInOutCubicEmphasized,
    decelerate: Cubic(0, 0, 0, 1),
    accelerate: Cubic(0.3, 0, 1, 1),
  );

  /// Hover and press feedback (120 ms).
  final Duration short;

  /// Most transitions (200 ms).
  final Duration medium;

  /// Drawers, sheets and pane changes (320 ms).
  final Duration long;

  /// Standard easing.
  final Curve standard;

  /// Emphasized easing for large movements.
  final Curve emphasized;

  /// Entering elements.
  final Curve decelerate;

  /// Exiting elements.
  final Curve accelerate;

  @override
  StrataMotion copyWith({
    Duration? short,
    Duration? medium,
    Duration? long,
    Curve? standard,
    Curve? emphasized,
    Curve? decelerate,
    Curve? accelerate,
  }) {
    return StrataMotion(
      short: short ?? this.short,
      medium: medium ?? this.medium,
      long: long ?? this.long,
      standard: standard ?? this.standard,
      emphasized: emphasized ?? this.emphasized,
      decelerate: decelerate ?? this.decelerate,
      accelerate: accelerate ?? this.accelerate,
    );
  }

  @override
  StrataMotion lerp(StrataMotion? other, double t) =>
      t < 0.5 ? this : (other ?? this);
}
