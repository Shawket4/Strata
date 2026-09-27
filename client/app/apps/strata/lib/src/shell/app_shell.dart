import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/providers.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// The navigation destinations, in shell-branch order.
enum AppDestination {
  /// Home / capture.
  home,

  /// Inbox.
  inbox,

  /// Tasks (hosted by Home on compact).
  tasks,

  /// Notes.
  notes,

  /// Global map (hosted by Notes on compact).
  map,

  /// People, companies, documents and places.
  directory,

  /// Ask.
  ask,

  /// Settings (footer of rail and sidebar; app-bar action on compact).
  settings,
}

/// Builds the [StrataDestination]s for [l10n] (SCREEN_SPEC "Additions":
/// compact bar = Home, Inbox, Notes, Directory, Ask).
List<StrataDestination> appDestinations(StrataLocalizations l10n) => [
  StrataDestination(
    icon: Icons.home_outlined,
    selectedIcon: Icons.home,
    label: l10n.navHome,
  ),
  StrataDestination(
    icon: Icons.inbox_outlined,
    selectedIcon: Icons.inbox,
    label: l10n.navInbox,
  ),
  StrataDestination(
    icon: Icons.check_circle_outline,
    selectedIcon: Icons.check_circle,
    label: l10n.navTasks,
    showInCompact: false,
    compactHostIndex: AppDestination.home.index,
  ),
  StrataDestination(
    icon: Icons.description_outlined,
    selectedIcon: Icons.description,
    label: l10n.navNotes,
  ),
  StrataDestination(
    icon: Icons.hub_outlined,
    selectedIcon: Icons.hub,
    label: l10n.navMap,
    showInCompact: false,
    compactHostIndex: AppDestination.notes.index,
  ),
  StrataDestination(
    icon: Icons.people_outline,
    selectedIcon: Icons.people,
    label: l10n.navDirectory,
  ),
  StrataDestination(
    icon: Icons.chat_bubble_outline,
    selectedIcon: Icons.chat_bubble,
    label: l10n.navAsk,
  ),
  StrataDestination(
    icon: Icons.settings_outlined,
    selectedIcon: Icons.settings,
    label: l10n.navSettings,
    placement: DestinationPlacement.footer,
  ),
];

/// Hosts the shell branches in the [AdaptiveScaffold].
class AppShell extends ConsumerWidget {
  /// Creates the shell for [navigationShell].
  const new({required this.navigationShell, super.key});

  /// go_router's stateful shell (one navigator per branch).
  final StatefulNavigationShell navigationShell;

  void _go(int index) => navigationShell.goBranch(
    index,
    initialLocation: index == navigationShell.currentIndex,
  );

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final destinations = appDestinations(l10n);
    final sync = ref.watch(syncStatusProvider).value;
    return AdaptiveScaffold(
      destinations: destinations,
      selectedIndex: navigationShell.currentIndex,
      onDestinationSelected: _go,
      title: destinations[navigationShell.currentIndex].label,
      navigationLabel: l10n.navigationLabel,
      capture: StrataAction(
        label: l10n.actionNewCapture,
        icon: Icons.add,
        onPressed: () => _go(AppDestination.home.index),
        shortcutKeys: const [KeyboardHintChip.commandKey, 'N'],
      ),
      appBarActions: [
        IconButton(
          tooltip: l10n.navSettings,
          icon: const Icon(Icons.settings_outlined),
          onPressed: () => _go(AppDestination.settings.index),
        ),
      ],
      syncIndicatorBuilder: sync == null
          ? null
          : (context, sizeClass) =>
                SyncPill(status: sync, dense: sizeClass == SizeClass.medium),
      body: navigationShell,
    );
  }
}
