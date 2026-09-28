import 'dart:async';

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
/// Tasks and Map) with the core's navigation counts ([NavView]).
List<StrataDestination> appDestinations(
  StrataLocalizations l10n, {
  NavView? nav,
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
    count: nav?.inboxCount,
  ),
  StrataDestination(
    icon: Icons.check_circle_outline,
    selectedIcon: Icons.check_circle,
    label: l10n.navTasks,
    count: nav?.tasksDueCount,
    showInCompact: false,
    compactHostIndex: AppDestination.home.index,
  ),
  StrataDestination(
    icon: Icons.description_outlined,
    selectedIcon: Icons.description,
    label: l10n.navNotes,
    count: nav?.notesCount,
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
    count: nav?.directoryCount,
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
/// core's counts (`watch_nav`), the sync pill / block opening the sync sheet,
/// drawer or popover, search (⌘K / Ctrl+K), pull-to-refresh on every screen
/// (and ⌘R / Ctrl+R), the pinned notes and folder tree on expanded, app
/// lifecycle forwarding and the reminders adapter.
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

  /// Pull-to-refresh: the core reconnects its live channel, syncs and re-reads
  /// the server-only data. A failure (offline) shows in the sync pill.
  Future<void> _refresh() async {
    try {
      await ref.read(coreApiProvider).refresh();
    } on Object {
      // The sync status says what went wrong.
    }
  }

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
    final nav = ref.watch(navProvider).value;
    final destinations = appDestinations(l10n, nav: nav);
    final pill = nav?.sync_;
    // A semantics boundary: the branch navigators' routes block the
    // semantics painted before them (the rail and sidebar) otherwise.
    final body = Semantics(
      container: true,
      explicitChildNodes: true,
      child: RefreshIndicator(
        // Any vertical list of any screen or pane, however deep.
        notificationPredicate: (n) => n.metrics.axis == Axis.vertical,
        onRefresh: _refresh,
        child: ScrollConfiguration(
          // Short lists can be pulled too.
          behavior: _PullableScroll(ScrollConfiguration.of(context)),
          child: KeyedSubtree(key: _bodyKey, child: widget.navigationShell),
        ),
      ),
    );
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
        search: StrataAction(
          label: l10n.actionSearch,
          icon: Icons.search,
          onPressed: () => const SearchRoute().go(context),
          shortcutKeys: const [KeyboardHintChip.commandKey, 'K'],
        ),
        appBarActions: [
          IconButton(
            tooltip: l10n.navSettings,
            icon: const Icon(Icons.settings_outlined),
            onPressed: () => _go(AppDestination.settings.index),
          ),
        ],
        sidebarSections: [
          if (sizeClass == SizeClass.expanded) ...[
            _PinnedNotes(pinned: nav?.pinned ?? const []),
            const _FolderTree(),
          ],
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
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.keyR, control: true): () =>
              unawaited(_refresh()),
          const SingleActivator(LogicalKeyboardKey.keyR, meta: true): () =>
              unawaited(_refresh()),
        },
        child: shell,
      ),
    );
  }
}

/// The ambient scroll behavior with lists that scroll (and so can be pulled)
/// even when their content fits.
class _PullableScroll extends ScrollBehavior {
  const new(this.base);

  final ScrollBehavior base;

  @override
  ScrollPhysics getScrollPhysics(BuildContext context) =>
      AlwaysScrollableScrollPhysics(parent: base.getScrollPhysics(context));

  @override
  Set<PointerDeviceKind> get dragDevices => base.dragDevices;

  @override
  Widget buildScrollbar(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) => base.buildScrollbar(context, child, details);

  @override
  Widget buildOverscrollIndicator(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) => base.buildOverscrollIndicator(context, child, details);
}

/// The sidebar's pinned notes (this device's pins, in pin order, from the
/// core's `NavView.pinned`).
class _PinnedNotes extends StatelessWidget {
  const new({required this.pinned});

  final List<NoteListItem> pinned;

  @override
  Widget build(BuildContext context) {
    if (pinned.isEmpty) return const SizedBox.shrink();
    final l10n = context.appL10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        StrataSectionHeader(title: l10n.pinnedNotesTitle),
        for (final note in pinned)
          _SidebarRow(
            icon: Icons.push_pin_outlined,
            label: note.title,
            labelDirection: textDirectionOf(note.titleDir),
            semanticsLabel: l10n.pinnedSemantics(title: note.title),
            onTap: () => NoteEditorRoute(noteId: note.id).go(context),
          ),
      ],
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        StrataSectionHeader(title: l10n.foldersTitle),
        for (final folder in folders)
          _SidebarRow(
            icon: Icons.folder_outlined,
            label: folder.name,
            count: folder.noteCount,
            semanticsLabel: l10n.folderSemantics(
              name: folder.name,
              count: folder.noteCount,
            ),
            onTap: () => NotesRoute(folder: folder.path).go(context),
          ),
      ],
    );
  }
}

/// One sidebar row of the pinned notes or the folder tree.
class _SidebarRow extends StatelessWidget {
  const new({
    required this.icon,
    required this.label,
    required this.semanticsLabel,
    required this.onTap,
    this.count,
    this.labelDirection,
  });

  final IconData icon;
  final String label;
  final String semanticsLabel;
  final VoidCallback onTap;
  final int? count;
  final TextDirection? labelDirection;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final n = count;
    return Semantics(
      container: true,
      button: true,
      label: semanticsLabel,
      excludeSemantics: true,
      child: InkWell(
        borderRadius: StrataRadii.inputRadius,
        onTap: onTap,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            minHeight: StrataLayout.minTapTarget(context).clamp(32, 48),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
            child: Row(
              children: [
                Icon(icon, size: 18, color: colors.text2),
                const SizedBox(width: StrataSpacing.s2),
                Expanded(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    textDirection: labelDirection,
                    textAlign: TextAlign.start,
                    style: text.label,
                  ),
                ),
                if (n != null)
                  Text('$n', style: text.caption.copyWith(color: colors.text2)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
