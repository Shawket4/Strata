import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:strata_ui/src/brand/strata_wordmark.dart';
import 'package:strata_ui/src/layout/destination.dart';
import 'package:strata_ui/src/layout/size_class.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';
import 'package:strata_ui/src/widgets/keyboard_hint_chip.dart';

/// Builds the sync indicator for a size class (e.g. a full pill in the app
/// bar and sidebar, a dense pill in the rail).
typedef SyncIndicatorBuilder = Widget Function(
  BuildContext context,
  SizeClass sizeClass,
);

/// Chrome labels are clamped to this text scale so navigation never clips;
/// content keeps the full system scale (PLAN §11: up to 200 %).
const double _maxChromeTextScale = 1.5;

/// The adaptive app shell: a bottom [NavigationBar] on compact, a scrollable
/// [NavigationRail] on medium and a [StrataSidebar] on expanded, chosen live
/// from the window width.
///
/// Destinations are supplied by the caller; `strata_ui` knows no features.
/// [selectedIndex] and [onDestinationSelected] use indices into the full
/// [destinations] list at every size class.
class AdaptiveScaffold extends StatelessWidget {
  /// Creates the shell.
  const new({
    required this.destinations,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.body,
    required this.title,
    required this.navigationLabel,
    super.key,
    this.capture,
    this.appBarActions = const [],
    this.syncIndicatorBuilder,
    this.sidebarSections = const [],
  }) : assert(
         selectedIndex >= 0 && selectedIndex < destinations.length,
         'selectedIndex must address a destination',
       );

  /// Every destination, in rail/sidebar order.
  final List<StrataDestination> destinations;

  /// Index of the current destination in [destinations].
  final int selectedIndex;

  /// Called with an index into [destinations].
  final ValueChanged<int> onDestinationSelected;

  /// The current screen.
  final Widget body;

  /// Compact app bar title.
  final String title;

  /// Accessibility label of the navigation region.
  final String navigationLabel;

  /// Capture action: top of the rail, "New capture" in the sidebar.
  final StrataAction? capture;

  /// Compact app bar actions (e.g. search).
  final List<Widget> appBarActions;

  /// Builds the always-visible sync indicator.
  final SyncIndicatorBuilder? syncIndicatorBuilder;

  /// Extra sidebar content below the navigation (pinned notes, folder tree).
  final List<Widget> sidebarSections;

  /// Indices of the destinations shown in the compact bottom bar.
  List<int> get compactIndices => [
    for (var i = 0; i < destinations.length; i++)
      if (destinations[i].showInCompact &&
          destinations[i].placement == DestinationPlacement.primary)
        i,
  ];

  /// Indices of the primary (non-footer) destinations.
  List<int> get primaryIndices => [
    for (var i = 0; i < destinations.length; i++)
      if (destinations[i].placement == DestinationPlacement.primary) i,
  ];

  /// Indices of the footer destinations.
  List<int> get footerIndices => [
    for (var i = 0; i < destinations.length; i++)
      if (destinations[i].placement == DestinationPlacement.footer) i,
  ];

  @override
  Widget build(BuildContext context) {
    final sizeClass = SizeClass.of(context);
    final shell = switch (sizeClass) {
      SizeClass.compact => _buildCompact(context),
      SizeClass.medium => _buildMedium(context),
      SizeClass.expanded => _buildExpanded(context),
    };
    final action = capture;
    if (action == null) return shell;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyN, meta: true):
            action.onPressed,
        const SingleActivator(LogicalKeyboardKey.keyN, control: true):
            action.onPressed,
      },
      child: shell,
    );
  }

  Widget _buildCompact(BuildContext context) {
    final indices = compactIndices;
    assert(
      indices.length <= StrataLayout.maxCompactDestinations,
      'A bottom navigation bar holds at most '
      '${StrataLayout.maxCompactDestinations} destinations.',
    );
    var selected = indices.indexOf(selectedIndex);
    if (selected < 0) {
      final host = destinations[selectedIndex].compactHostIndex;
      selected = host == null ? 0 : indices.indexOf(host);
      if (selected < 0) selected = 0;
    }
    final sync = syncIndicatorBuilder?.call(context, SizeClass.compact);
    return Scaffold(
      appBar: AppBar(
        title: Text(title),
        actions: [
          ...appBarActions,
          if (sync != null)
            Padding(
              padding: const EdgeInsetsDirectional.only(end: StrataSpacing.s3),
              child: Center(
                // Leaves the title room at large text scales; the pill
                // ellipsizes and keeps its full semantics label.
                child: ConstrainedBox(
                  constraints: BoxConstraints(
                    maxWidth: MediaQuery.sizeOf(context).width * 0.45,
                  ),
                  child: sync,
                ),
              ),
            ),
        ],
      ),
      body: body,
      bottomNavigationBar: Semantics(
        container: true,
        label: navigationLabel,
        explicitChildNodes: true,
        child: MediaQuery.withClampedTextScaling(
          maxScaleFactor: _maxChromeTextScale,
          child: NavigationBar(
            selectedIndex: selected,
            onDestinationSelected: (i) => onDestinationSelected(indices[i]),
            labelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
            destinations: [
              for (final i in indices)
                NavigationDestination(
                  icon: Icon(destinations[i].icon),
                  selectedIcon: Icon(
                    destinations[i].selectedIcon ?? destinations[i].icon,
                  ),
                  label: destinations[i].label,
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildMedium(BuildContext context) {
    final colors = context.strataColors;
    final primary = primaryIndices;
    final railSelected = primary.indexOf(selectedIndex);
    final sync = syncIndicatorBuilder?.call(context, SizeClass.medium);
    final action = capture;
    return Scaffold(
      body: Row(
        children: [
          Semantics(
            container: true,
            label: navigationLabel,
            explicitChildNodes: true,
            child: MediaQuery.withClampedTextScaling(
              maxScaleFactor: _maxChromeTextScale,
              child: NavigationRail(
                scrollable: true,
                selectedIndex: railSelected < 0 ? null : railSelected,
                onDestinationSelected: (i) => onDestinationSelected(primary[i]),
                leading: action == null
                    ? null
                    : Padding(
                        padding: const EdgeInsets.only(
                          top: StrataSpacing.s2,
                          bottom: StrataSpacing.s3,
                        ),
                        child: FloatingActionButton(
                          heroTag: null,
                          tooltip: action.label,
                          onPressed: action.onPressed,
                          child: Icon(action.icon),
                        ),
                      ),
                trailingAtBottom: true,
                trailing: Padding(
                  padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      for (final i in footerIndices)
                        RailFooterDestination(
                          destination: destinations[i],
                          selected: i == selectedIndex,
                          onTap: () => onDestinationSelected(i),
                        ),
                      if (sync != null) ...[
                        const SizedBox(height: StrataSpacing.s2),
                        sync,
                      ],
                    ],
                  ),
                ),
                destinations: [
                  for (final i in primary)
                    NavigationRailDestination(
                      icon: Icon(destinations[i].icon),
                      selectedIcon: Icon(
                        destinations[i].selectedIcon ?? destinations[i].icon,
                      ),
                      label: RailLabel(destinations[i].label),
                    ),
                ],
              ),
            ),
          ),
          VerticalDivider(width: 1, color: colors.border),
          Expanded(child: body),
        ],
      ),
    );
  }

  Widget _buildExpanded(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          StrataSidebar(
            destinations: destinations,
            selectedIndex: selectedIndex,
            onDestinationSelected: onDestinationSelected,
            navigationLabel: navigationLabel,
            capture: capture,
            sections: sidebarSections,
            syncIndicator: syncIndicatorBuilder?.call(
              context,
              SizeClass.expanded,
            ),
          ),
          VerticalDivider(width: 1, color: context.strataColors.border),
          Expanded(child: body),
        ],
      ),
    );
  }
}

/// A footer destination (e.g. Settings) at the bottom of the navigation rail,
/// drawn like a rail destination.
class RailFooterDestination extends StatelessWidget {
  /// Creates the footer destination.
  const new({
    required this.destination,
    required this.selected,
    required this.onTap,
    super.key,
  });

  /// The destination.
  final StrataDestination destination;

  /// Whether it is the current destination.
  final bool selected;

  /// Called when activated.
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final fg = selected ? colors.accentText : colors.text2;
    return Semantics(
      container: true,
      button: true,
      selected: selected,
      label: destination.semanticLabel,
      onTap: onTap,
      excludeSemantics: true,
      child: InkWell(
        onTap: onTap,
        borderRadius: StrataRadii.cardRadius,
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minWidth: StrataLayout.railWidth,
            maxWidth: StrataLayout.railWidth,
            minHeight: StrataLayout.minTouchTarget + StrataSpacing.s4,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 56,
                  height: 32,
                  decoration: BoxDecoration(
                    color: selected ? colors.accentTint : null,
                    borderRadius: StrataRadii.pillRadius,
                  ),
                  child: Icon(
                    selected
                        ? destination.selectedIcon ?? destination.icon
                        : destination.icon,
                    color: fg,
                    size: 22,
                  ),
                ),
                const SizedBox(height: StrataSpacing.s1),
                RailLabel(
                  destination.label,
                  style: text.caption
                      .withWeight(selected ? FontWeight.w600 : FontWeight.w400)
                      .copyWith(color: fg),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The expanded-layout sidebar (248 px): wordmark row, "New capture" button,
/// scrollable navigation with counts, caller-provided sections (pinned notes,
/// folder tree), footer destinations and the sync status block.
class StrataSidebar extends StatelessWidget {
  /// Creates the sidebar.
  const new({
    required this.destinations,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.navigationLabel,
    super.key,
    this.capture,
    this.sections = const [],
    this.syncIndicator,
  });

  /// Every destination.
  final List<StrataDestination> destinations;

  /// Index of the current destination.
  final int selectedIndex;

  /// Called with an index into [destinations].
  final ValueChanged<int> onDestinationSelected;

  /// Accessibility label of the navigation region.
  final String navigationLabel;

  /// "New capture" action.
  final StrataAction? capture;

  /// Extra scrollable sections below the navigation.
  final List<Widget> sections;

  /// Sync status block pinned to the bottom.
  final Widget? syncIndicator;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final action = capture;
    final sync = syncIndicator;
    SidebarItem item(int i) => SidebarItem(
      destination: destinations[i],
      selected: i == selectedIndex,
      onTap: () => onDestinationSelected(i),
    );
    return Material(
      color: colors.surface2,
      child: SizedBox(
        width: StrataLayout.sidebarWidth,
        child: SafeArea(
          right: false,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Padding(
                padding: EdgeInsetsDirectional.fromSTEB(
                  StrataSpacing.s4,
                  StrataSpacing.s5,
                  StrataSpacing.s4,
                  StrataSpacing.s3,
                ),
                child: Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: StrataWordmark(fontSize: 20),
                ),
              ),
              if (action != null)
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s3,
                    vertical: StrataSpacing.s1,
                  ),
                  child: _CaptureButton(action: action),
                ),
              Expanded(
                child: Semantics(
                  container: true,
                  label: navigationLabel,
                  explicitChildNodes: true,
                  child: ListView(
                    padding: const EdgeInsets.all(StrataSpacing.s2),
                    children: [
                      for (var i = 0; i < destinations.length; i++)
                        if (destinations[i].placement ==
                            DestinationPlacement.primary)
                          item(i),
                      ...sections,
                    ],
                  ),
                ),
              ),
              Divider(height: 1, color: colors.border),
              Padding(
                padding: const EdgeInsets.all(StrataSpacing.s2),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (var i = 0; i < destinations.length; i++)
                      if (destinations[i].placement ==
                          DestinationPlacement.footer)
                        item(i),
                    if (sync != null)
                      Padding(
                        padding: const EdgeInsets.all(StrataSpacing.s2),
                        child: Align(
                          alignment: AlignmentDirectional.centerStart,
                          child: sync,
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CaptureButton extends StatelessWidget {
  const new({required this.action});

  final StrataAction action;

  @override
  Widget build(BuildContext context) {
    final keys = action.shortcutKeys;
    return FilledButton(
      onPressed: action.onPressed,
      style: FilledButton.styleFrom(
        minimumSize: Size(0, StrataLayout.minTapTarget(context)),
        padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
      ),
      child: Row(
        children: [
          Icon(action.icon, size: 20),
          const SizedBox(width: StrataSpacing.s2),
          Expanded(
            child: Text(
              action.label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          if (keys != null) KeyboardHintChip(keys: keys, onAccent: true),
        ],
      ),
    );
  }
}

/// One sidebar navigation row: icon, label and optional count.
class SidebarItem extends StatelessWidget {
  /// Creates a sidebar row.
  const new({
    required this.destination,
    required this.selected,
    required this.onTap,
    super.key,
  });

  /// The destination.
  final StrataDestination destination;

  /// Whether it is the current destination.
  final bool selected;

  /// Called when activated.
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final fg = selected ? colors.accentText : colors.text;
    final count = destination.count;
    return Semantics(
      container: true,
      button: true,
      selected: selected,
      label: destination.semanticLabel,
      onTap: onTap,
      excludeSemantics: true,
      child: Material(
        color: selected ? colors.accentTint : Colors.transparent,
        borderRadius: StrataRadii.inputRadius,
        child: InkWell(
          onTap: onTap,
          borderRadius: StrataRadii.inputRadius,
          hoverColor: colors.border.withValues(alpha: 0.35),
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: StrataLayout.minTapTarget(context).clamp(40, 48),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: StrataSpacing.s3,
                vertical: StrataSpacing.s1,
              ),
              child: Row(
                children: [
                  Icon(
                    selected
                        ? destination.selectedIcon ?? destination.icon
                        : destination.icon,
                    size: 20,
                    color: selected ? colors.accentText : colors.text2,
                  ),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(
                    child: Text(
                      destination.label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: text.label
                          .withWeight(
                            selected ? FontWeight.w600 : FontWeight.w500,
                          )
                          .copyWith(color: fg),
                    ),
                  ),
                  if (count != null)
                    Text(
                      '$count',
                      style: text.caption
                          .withWeight(FontWeight.w600)
                          .copyWith(
                            color: selected ? colors.accentText : colors.text2,
                          ),
                    ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// A navigation-rail label: one line, scaled down to fit the 80 px rail
/// rather than wrapping mid-word.
class RailLabel extends StatelessWidget {
  /// Creates a rail label.
  const new(this.label, {super.key, this.style});

  /// The label text.
  final String label;

  /// Optional style (the rail theme's label style applies otherwise).
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: StrataLayout.railWidth - StrataSpacing.s4,
      child: FittedBox(
        fit: BoxFit.scaleDown,
        child: Text(label, maxLines: 1, style: style),
      ),
    );
  }
}
