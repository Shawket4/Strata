import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:strata/strata.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'helpers/boot.dart';
import 'helpers/matrix.dart';

void main() {
  group('shell matrix', () {
    for (final v in matrix()) {
      testWidgets('navigation and sync pill $v', (tester) async {
        final l10n = await StrataLocalizations.delegate.load(v.locale);
        final sync = lookupSyncLocalizations(v.locale);
        final app = await boot(
          tester,
          size: v.size,
          brightness: v.brightness,
          locale: v.locale,
          textScale: v.textScale,
        );
        // Settings is this package's own content; a11y is checked there.
        await go(tester, app, const SettingsRoute().location);
        final context = tester.element(find.byType(AdaptiveScaffold));
        expect(Theme.of(context).brightness, v.brightness);
        expect(
          Directionality.of(context),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expect(find.byType(SettingsScreen), findsOneWidget);
        final pill = tester.widget<SyncStatusPill>(find.byType(SyncStatusPill));
        expect(pill.pill, StrataFixtures.syncStatusView.pill);
        expect(pill.dense, v.sizeClass == SizeClass.medium);
        final label = sync.pillWithConflicts(
          status: sync.pillOnline(count: 1),
          conflicts: sync.pillConflicts(count: 1),
        );
        expect(
          find.text(label),
          v.sizeClass == SizeClass.medium ? findsNothing : findsOneWidget,
        );
        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(NavigationDestination), findsNWidgets(5));
            expect(find.byTooltip(l10n.navSettings), findsOneWidget);
          case SizeClass.medium:
            final rail = tester.widget<NavigationRail>(
              find.byType(NavigationRail),
            );
            expect(rail.destinations, hasLength(7));
            expect(find.byType(RailFooterDestination), findsOneWidget);
          case SizeClass.expanded:
            expect(find.byType(SidebarItem), findsNWidgets(8));
            final inbox = tester
                .widgetList<SidebarItem>(find.byType(SidebarItem))
                .firstWhere((i) => i.destination.label == l10n.navInbox);
            expect(inbox.destination.count, StrataFixtures.homeView.inboxCount);
            expect(find.text(l10n.actionNewCapture), findsOneWidget);
        }
        expectNoErrors(tester);
        // strata_ui's RailFooterDestination scales an RTL label down in a
        // FittedBox; the guideline samples the unscaled paint bounds there
        // (reported to strata_ui). Every other variant checks contrast.
        await expectAccessible(
          tester,
          contrast:
              v.textScale == 1 && !(v.sizeClass == SizeClass.medium && v.rtl),
        );
      });
    }
  });

  group('navigation', () {
    testWidgets('bottom bar switches branches; settings from the app bar', (
      tester,
    ) async {
      final app = await boot(tester);
      expect(app.router.state.uri.path, '/home');
      await tester.tap(find.text('Inbox'));
      await settle(tester);
      expect(app.router.state.uri.path, '/inbox');
      await tester.tap(find.text('Ask'));
      await settle(tester);
      expect(app.router.state.uri.path, '/ask');
      await tester.tap(find.text('Directory'));
      await settle(tester);
      expect(app.router.state.uri.path, '/directory');
      await tester.tap(find.byTooltip('Settings'));
      await settle(tester);
      expect(app.router.state.uri.path, '/settings');
      expect(find.byType(SettingsScreen), findsOneWidget);
    });

    testWidgets('the rail reaches Tasks and Map', (tester) async {
      final app = await boot(tester, size: StrataTestSizes.medium);
      await tester.tap(find.text('Map'));
      await settle(tester);
      expect(app.router.state.uri.path, '/map');
      await tester.tap(find.text('Tasks'));
      await settle(tester);
      expect(app.router.state.uri.path, '/tasks');
      await tester.tap(find.byType(RailFooterDestination));
      await settle(tester);
      expect(app.router.state.uri.path, '/settings');
    });

    testWidgets('the sidebar and its folder tree', (tester) async {
      final fake = FakeCoreApi()
        ..notesList[''].add(StrataFixtures.notesListView);
      final app = await boot(
        tester,
        fake: fake,
        size: StrataTestSizes.expanded,
      );
      // (Settings: this package's own content next to the sidebar.)
      await go(tester, app, const SettingsRoute().location);
      final folder = StrataFixtures.notesListView.folders.first;
      expect(find.text(folder.name), findsWidgets);
      await tester.tap(
        find.bySemanticsLabel('${folder.name}, ${folder.noteCount} notes'),
      );
      await settle(tester);
      expect(app.router.state.uri.path, '/notes');
      await tester.tap(find.byType(SidebarItem).at(2));
      await settle(tester);
      expect(app.router.state.uri.path, '/tasks');
    });

    testWidgets('Cmd/Ctrl-N opens capture (Home)', (tester) async {
      final app = await boot(tester, size: StrataTestSizes.expanded);
      await go(tester, app, '/inbox');
      await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
      await settle(tester);
      expect(app.router.state.uri.path, '/home');
    });

    for (final (size, surface) in [
      (StrataTestSizes.compact, BottomSheet),
      (StrataTestSizes.medium, SyncDrawer),
      (StrataTestSizes.expanded, SyncPopover),
    ]) {
      testWidgets('the sync pill opens the $surface', (tester) async {
        final app = await boot(tester, size: size);
        await go(tester, app, '/settings');
        await tester.tap(find.byType(SyncStatusPill));
        await settle(tester);
        expect(find.byType(surface), findsOneWidget);
        // Review closes the surface and opens the conflict.
        await tapVisible(tester, find.text('Review'));
        expect(find.byType(surface), findsNothing);
        expect(
          app.router.state.uri.path,
          ConflictRoute(opId: StrataFixtures.conflictItem.opId).location,
        );
        expect(find.byType(ConflictResolutionScreen), findsOneWidget);
      });
    }

    testWidgets('app lifecycle is forwarded', (tester) async {
      final app = await boot(tester);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      await tester.pump();
      expect(app.fake.calls.where((c) => c.method == 'appLifecycle'), [
        const CoreCall('appLifecycle', {'state': AppLifecycle.paused}),
        const CoreCall('appLifecycle', {'state': AppLifecycle.resumed}),
      ]);
    });
  });

  group('deep links', () {
    testWidgets('a note on expanded stays in the shell under Notes', (
      tester,
    ) async {
      final app = await boot(tester, size: StrataTestSizes.expanded);
      await go(tester, app, const NoteEditorRoute(noteId: 'n-1').location);
      expect(find.byType(AdaptiveScaffold), findsOneWidget);
      final notes = tester
          .widgetList<SidebarItem>(find.byType(SidebarItem))
          .firstWhere((i) => i.destination.label == 'Notes');
      expect(notes.selected, isTrue);
    });

    testWidgets('details are full-screen on compact', (tester) async {
      final app = await boot(tester);
      for (final location in [
        const TaskRoute(taskId: 't-petrol-arrows').location,
        const NoteEditorRoute(noteId: 'n-1').location,
        const ConflictRoute(opId: 'op-1').location,
        const EntityRoute(entityId: 'p-ahmed').location,
      ]) {
        await go(tester, app, location);
        expect(app.router.state.uri.path, location);
        expect(find.byType(NavigationBar), findsNothing, reason: location);
      }
      await go(tester, app, const InboxRoute().location);
      expect(find.byType(NavigationBar), findsOneWidget);
    });

    testWidgets('settings sections and Admin → Users per size class', (
      tester,
    ) async {
      final app = await boot(tester);
      await go(
        tester,
        app,
        const SettingsSectionRoute(section: 'reminders').location,
      );
      expect(find.byType(RemindersSection), findsOneWidget);
      await tester.tap(find.byType(BackButton));
      await settle(tester);
      expect(app.router.state.uri.path, '/settings');
      await go(tester, app, const AdminUsersRoute().location);
      expect(find.byType(AdminUsersScreen), findsOneWidget);
      expect(find.byType(SettingsScreen), findsNothing);
      tester.view.physicalSize = StrataTestSizes.expanded;
      await settle(tester);
      // Wider: Admin → Users is the Settings content pane.
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(find.byType(AdminUsersScreen), findsOneWidget);
      await tapVisible(tester, find.text('About'));
      expect(app.router.state.uri.path, '/settings/about');
    });

    testWidgets('/sync is a page outside the shell', (tester) async {
      final app = await boot(tester, size: StrataTestSizes.expanded);
      await go(tester, app, const SyncRoute().location);
      expect(find.byType(SyncScreen), findsOneWidget);
      expect(find.byType(AdaptiveScaffold), findsNothing);
    });

    testWidgets('unknown places lead home', (tester) async {
      final app = await boot(tester);
      await go(tester, app, '/does-not-exist');
      expect(app.router.state.uri.path, '/home');
    });
  });

  group('resize', () {
    testWidgets('branch stacks and state survive every size class', (
      tester,
    ) async {
      final app = await boot(tester, size: StrataTestSizes.expanded);
      await go(
        tester,
        app,
        const TaskRoute(taskId: 't-petrol-arrows').location,
      );
      await go(tester, app, const InboxRoute().location);
      final shellState = tester.state(find.byType(StatefulNavigationShell));
      for (final size in [
        StrataTestSizes.medium,
        StrataTestSizes.compact,
        StrataTestSizes.large,
        StrataTestSizes.expanded,
      ]) {
        tester.view.physicalSize = size;
        await settle(tester);
        expect(app.router.state.uri.path, '/inbox');
        expect(
          tester.state(find.byType(StatefulNavigationShell)),
          same(shellState),
          reason: '$size',
        );
        expectNoErrors(tester);
      }
      // Back to Tasks: its detail is still on top of the branch.
      await tester.tap(find.byType(SidebarItem).at(2));
      await settle(tester);
      expect(app.router.state.uri.path, '/tasks/t-petrol-arrows');
    });

    testWidgets('a compact full-screen detail keeps the shell state', (
      tester,
    ) async {
      final app = await boot(tester);
      final shellState = tester.state(find.byType(StatefulNavigationShell));
      await go(tester, app, const TaskRoute(taskId: 't-1').location);
      expect(find.byType(NavigationBar), findsNothing);
      tester.view.physicalSize = StrataTestSizes.expanded;
      await settle(tester);
      expect(find.byType(StrataSidebar), findsOneWidget);
      expect(
        tester.state(find.byType(StatefulNavigationShell)),
        same(shellState),
      );
    });
  });

  group('typed routes', () {
    test('locations', () {
      expect(const HomeRoute().location, '/home');
      expect(const InboxRoute().location, '/inbox');
      expect(const InboxItemRoute(noteId: 'c-1').location, '/inbox/c-1');
      expect(const TasksRoute().location, '/tasks');
      expect(const TaskRoute(taskId: 't-1').location, '/tasks/t-1');
      expect(const NotesRoute().location, '/notes');
      expect(
        const NotesRoute(folder: 'notes/sales').location,
        '/notes?folder=notes%2Fsales',
      );
      expect(
        const NoteEditorRoute(noteId: 'Call 2026-09-12').location,
        '/notes/Call%202026-09-12',
      );
      expect(const MindMapRoute(noteId: 'n-1').location, '/notes/n-1/map');
      expect(
        const ConflictRoute(opId: 'op-1').location,
        '/notes/conflicts/op-1',
      );
      expect(const MapRoute().location, '/map');
      expect(const DirectoryRoute().location, '/directory');
      expect(
        const DirectoryRoute(tab: DirectoryTab.places).location,
        '/directory?tab=places',
      );
      expect(const EntityRoute(entityId: 'p-1').location, '/directory/p-1');
      expect(
        const DocumentRoute(documentId: 'd-1').location,
        '/directory/documents/d-1',
      );
      expect(
        const PlaceRoute(placeId: 'pl-1').location,
        '/directory/places/pl-1',
      );
      expect(const AskRoute().location, '/ask');
      expect(const SettingsRoute().location, '/settings');
      expect(
        const SettingsSectionRoute(section: 'devices').location,
        '/settings/devices',
      );
      expect(const AdminUsersRoute().location, '/settings/admin/users');
      expect(const SyncRoute().location, '/sync');
      expect(const SignInRoute().location, '/sign-in');
      expect(const SignUpRoute(server: 'x').location, '/sign-up?server=x');
      expect(const ApprovalRoute().location, '/approval');
      expect(const AccountDisabledRoute().location, '/account-disabled');
      expect(const DeletionPendingRoute().location, '/deletion-pending');
      expect(const PasswordChangeRoute().location, '/password-change');
    });

    test('shell branches follow the destination order', () {
      final shell = $appRoutes.first as StatefulShellRoute;
      expect(shell.branches, hasLength(AppDestination.values.length));
      expect(
        [
          for (final branch in shell.branches)
            (branch.routes.first as GoRoute).path,
        ],
        [
          '/home',
          '/inbox',
          '/tasks',
          '/notes',
          '/map',
          '/directory',
          '/ask',
          '/settings',
        ],
      );
    });

    test('destinations mirror the navigation update', () async {
      final l10n = await StrataLocalizations.delegate.load(
        StrataLocales.english,
      );
      final all = appDestinations(l10n, inboxCount: 4);
      expect(all.map((d) => d.label), [
        'Home',
        'Inbox',
        'Tasks',
        'Notes',
        'Map',
        'Directory',
        'Ask',
        'Settings',
      ]);
      expect(all[1].count, 4);
      expect(
        all
            .where(
              (d) =>
                  d.showInCompact &&
                  d.placement == DestinationPlacement.primary,
            )
            .map((d) => d.label),
        ['Home', 'Inbox', 'Notes', 'Directory', 'Ask'],
      );
    });

    test('session places', () {
      Uri u(String path) => Uri.parse(path);
      const active = StrataFixtures.sessionActive;
      expect(sessionRedirect(null, u('/anything')), isNull);
      expect(sessionRedirect(active, u('/notes/n-1')), isNull);
      expect(sessionRedirect(active, u('/sign-in')), '/home');
      expect(
        sessionRedirect(StrataFixtures.sessionSignedOut, u('/settings')),
        '/sign-in',
      );
      expect(
        sessionRedirect(StrataFixtures.sessionSignedOut, u('/approval')),
        isNull,
      );
      expect(isDetailLocation(u('/tasks/t-1')), isTrue);
      expect(isDetailLocation(u('/tasks')), isFalse);
      expect(platformOf(TargetPlatform.iOS), Platform.ios);
      expect(platformOf(TargetPlatform.fuchsia), Platform.linux);
    });
  });
}
