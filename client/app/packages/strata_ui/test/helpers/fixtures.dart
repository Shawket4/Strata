import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Index constants of [testDestinations] (the app's navigation, per the
/// design spec "Additions": compact = Home, Inbox, Notes, Directory, Ask).
abstract final class Dest {
  static const home = 0;
  static const inbox = 1;
  static const tasks = 2;
  static const notes = 3;
  static const map = 4;
  static const directory = 5;
  static const ask = 6;
  static const settings = 7;
}

/// The eight destinations the app shell passes to [AdaptiveScaffold].
List<StrataDestination> testDestinations(StrataLocalizations l10n) => [
  StrataDestination(
    icon: Icons.home_outlined,
    selectedIcon: Icons.home,
    label: l10n.navHome,
  ),
  StrataDestination(
    icon: Icons.inbox_outlined,
    selectedIcon: Icons.inbox,
    label: l10n.navInbox,
    count: 4,
  ),
  StrataDestination(
    icon: Icons.check_circle_outline,
    selectedIcon: Icons.check_circle,
    label: l10n.navTasks,
    count: 2,
    showInCompact: false,
    compactHostIndex: Dest.home,
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
    compactHostIndex: Dest.notes,
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

/// A sample shell with every slot filled, as the app uses it.
class TestShell extends StatelessWidget {
  const new({
    this.selectedIndex = Dest.inbox,
    this.onSelected,
    this.onCapture,
    this.body,
    this.sync = const SyncOffline(queued: 3),
    super.key,
  });

  final int selectedIndex;
  final ValueChanged<int>? onSelected;
  final VoidCallback? onCapture;
  final Widget? body;
  final SyncStatus sync;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AdaptiveScaffold(
      destinations: testDestinations(l10n),
      selectedIndex: selectedIndex,
      onDestinationSelected: onSelected ?? (_) {},
      title: l10n.navInbox,
      navigationLabel: l10n.navigationLabel,
      capture: StrataAction(
        label: l10n.actionNewCapture,
        icon: Icons.add,
        onPressed: onCapture ?? () {},
        shortcutKeys: const ['⌘', 'N'],
      ),
      appBarActions: [
        IconButton(
          tooltip: l10n.actionSearch,
          icon: const Icon(Icons.search),
          onPressed: () {},
        ),
      ],
      syncIndicatorBuilder: (context, sizeClass) =>
          SyncPill(status: sync, dense: sizeClass == SizeClass.medium),
      sidebarSections: [StrataSectionHeader(title: l10n.navNotes, count: 2)],
      body: body ?? const SamplePanes(),
    );
  }
}

/// A list / detail / context pane body used by the shell tests and goldens.
class SamplePanes extends StatelessWidget {
  const new({this.contextOpen = false, super.key});

  final bool contextOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final text = context.strataText;
    return StrataPanes(
      contextPanelOpen: contextOpen,
      onContextPanelClosed: () {},
      contextPanelLabel: l10n.contextPanelLabel,
      dismissLabel: l10n.actionClose,
      list: ListView(
        children: [
          StrataSectionHeader(title: l10n.navInbox, count: 3),
          for (final title in const [
            'Weekly invoicing request — Acme',
            'Pricing experiments',
            'تجارب التسعير — ملخص',
          ])
            ListTile(title: Text(title, style: text.bodySmall)),
        ],
      ),
      detail: StrataEmptyState(
        title: l10n.navInbox,
        message: l10n.featurePlaceholderMessage,
      ),
      contextPanel: ListView(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        children: [
          StrataSectionHeader(title: l10n.contextPanelLabel),
          const Wrap(
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s2,
            children: [
              RelationChip(
                type: RelationType.followsUp,
                label: 'Call 2026-09-12 — Acme',
                aiConfidence: 0.82,
              ),
              RelationChip(
                type: RelationType.partOf,
                label: 'Subscription tiers',
              ),
            ],
          ),
        ],
      ),
    );
  }
}
