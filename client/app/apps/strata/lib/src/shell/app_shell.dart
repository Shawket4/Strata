import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/l10n.dart';
import 'package:strata/src/reminders/reminder_adapter.dart';
import 'package:strata/src/router/routes.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

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
/// compact bar = Home, Inbox, Notes, Directory, Ask; rail and sidebar add
/// Tasks and Map). [inboxCount] is the core's `HomeView.inboxCount`.
List<StrataDestination> appDestinations(
  StrataLocalizations l10n, {
  int? inboxCount,
}) => [
  StrataDestination(
    icon: Icons.home_outlined,
    selectedIcon: Icons.home,
    label: l10n.navHome,
  ),
  StrataDestination(
    icon: Icons.inbox_outlined,
    selectedIcon: Icons.inbox,
    label: l10n.navInbox,
    count: inboxCount,
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

/// Whether [location] is a detail screen of its branch (more than one path
/// segment), shown full-screen on compact (PLAN §11 "detail pushes
/// full-screen").
bool isDetailLocation(Uri location) => location.pathSegments.length > 1;

/// Hosts the shell branches in the [AdaptiveScaffold]: destinations with the
/// core's counts, the sync pill / block opening the sync sheet, drawer or
/// popover, the folder tree on expanded, app lifecycle forwarding and the
/// reminders adapter.
///
/// The branch navigators live under one [GlobalKey], so their state (stacks,
/// scroll positions, text being typed) survives every change of size class
/// and the switch to a full-screen detail on compact.
class AppShell extends ConsumerStatefulWidget {
  /// Creates the shell for [navigationShell] at [location].
  const new({required this.navigationShell, required this.location, super.key});

  /// go_router's stateful shell (one navigator per branch).
  final StatefulNavigationShell navigationShell;

  /// The current location (full-screen details on compact).
  final Uri location;

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  final GlobalKey _bodyKey = GlobalKey(debugLabel: 'shell-body');
  late final AppLifecycleListener _lifecycle;

  @override
  void initState() {
    super.initState();
    _lifecycle = AppLifecycleListener(
      onResume: () => _lifecycleEvent(AppLifecycle.resumed),
      onPause: () => _lifecycleEvent(AppLifecycle.paused),
    );
  }

  void _lifecycleEvent(AppLifecycle state) =>
      unawaited(ref.read(coreApiProvider).appLifecycle(state: state));

  @override
  void dispose() {
    _lifecycle.dispose();
    super.dispose();
  }

  void _go(int index) => widget.navigationShell.goBranch(
    index,
    initialLocation: index == widget.navigationShell.currentIndex,
  );

  void _openSync() => unawaited(
    showSyncStatus(
      context,
      onOpenConflict: (opId) => ConflictRoute(opId: opId).go(context),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final sizeClass = SizeClass.of(context);
    final inbox = ref.watch(homeProvider).value?.inboxCount;
    final destinations = appDestinations(l10n, inboxCount: inbox);
    final pill = ref.watch(syncStatusProvider).value?.pill;
    final body = KeyedSubtree(key: _bodyKey, child: widget.navigationShell);
    final Widget shell;
    if (sizeClass == SizeClass.compact && isDetailLocation(widget.location)) {
      // The detail's own page chrome (app bar, back) fills the window.
      shell = Material(child: body);
    } else {
      shell = AdaptiveScaffold(
        destinations: destinations,
        selectedIndex: widget.navigationShell.currentIndex,
        onDestinationSelected: _go,
        title: destinations[widget.navigationShell.currentIndex].label,
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
        sidebarSections: [
          if (sizeClass == SizeClass.expanded) const _FolderTree(),
        ],
        syncIndicatorBuilder: pill == null
            ? null
            : (context, sizeClass) => SyncStatusPill(
                pill: pill,
                dense: sizeClass == SizeClass.medium,
                onPressed: _openSync,
              ),
        body: body,
      );
    }
    return ReminderAdapterHost(
      onOpenTask: (taskId) => TaskRoute(taskId: taskId).go(context),
      child: shell,
    );
  }
}

/// The sidebar's folder tree: the vault root's folders with their note
/// counts, from the core's notes list.
class _FolderTree extends ConsumerWidget {
  const new();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final folders = ref.watch(notesListProvider('')).value?.folders;
    if (folders == null || folders.isEmpty) return const SizedBox.shrink();
    final l10n = context.appL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        StrataSectionHeader(title: l10n.pinnedTitle),
        for (final folder in folders)
          Semantics(
            container: true,
            button: true,
            label: l10n.folderSemantics(
              name: folder.name,
              count: folder.noteCount,
            ),
            excludeSemantics: true,
            child: InkWell(
              borderRadius: StrataRadii.inputRadius,
              onTap: () => const NotesRoute().go(context),
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  minHeight: StrataLayout.minTapTarget(context).clamp(32, 48),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s3,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        Icons.folder_outlined,
                        size: 18,
                        color: colors.text2,
                      ),
                      const SizedBox(width: StrataSpacing.s2),
                      Expanded(
                        child: Text(
                          folder.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: text.label,
                        ),
                      ),
                      Text(
                        '${folder.noteCount}',
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }
}
