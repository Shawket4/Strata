import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/strata.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/strata_maps.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// Boots the real app (ProviderScope + MaterialApp.router) with the core's
/// sync stream replaced by a fixture, at [size] / [brightness] / [locale] /
/// [textScale].
Future<void> _boot(
  WidgetTester tester, {
  Size size = StrataTestSizes.compact,
  Brightness brightness = Brightness.light,
  Locale locale = StrataLocales.english,
  double textScale = 1,
  SyncStatus? sync = const SyncOffline(queued: 3),
}) async {
  tester.view
    ..physicalSize = size
    ..devicePixelRatio = 1;
  tester.platformDispatcher
    ..platformBrightnessTestValue = brightness
    ..textScaleFactorTestValue = textScale;
  addTearDown(tester.view.reset);
  addTearDown(tester.platformDispatcher.clearAllTestValues);
  final container = ProviderContainer.test(
    overrides: [
      syncStatusProvider.overrideWith(
        (ref) => sync == null ? const Stream.empty() : Stream.value(sync),
      ),
    ],
  );
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: StrataApp(locale: locale),
    ),
  );
  await tester.pumpAndSettle();
}

/// The app's router, for deep-link assertions.
GoRouter _router(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(StrataApp)))
        .read(appRouterProvider);

void main() {
  group('app shell matrix', () {
    for (final size in StrataTestSizes.all.entries) {
      for (final brightness in Brightness.values) {
        for (final locale in StrataLocalizations.supportedLocales) {
          for (final scale in const [1.0, 2.0]) {
            final id =
                '${size.key}_${brightness.name}_${locale.languageCode}_'
                '${scale}x';
            testWidgets(id, (tester) async {
              await _boot(
                tester,
                size: size.value,
                brightness: brightness,
                locale: locale,
                textScale: scale,
              );
              final l10n = await StrataLocalizations.delegate.load(locale);
              final context = tester.element(find.byType(HomeScreen));

              expect(Theme.of(context).brightness, brightness);
              expect(
                Directionality.of(context),
                locale.languageCode == 'ar'
                    ? TextDirection.rtl
                    : TextDirection.ltr,
              );
              expect(find.byType(HomeScreen), findsOneWidget);
              expect(find.text(l10n.navHome), findsWidgets);
              expect(find.byType(AdaptiveScaffold), findsOneWidget);
              expect(
                find.text(l10n.syncOfflineQueued(count: 3)),
                SizeClass.fromWidth(size.value.width) == SizeClass.medium
                    ? findsNothing
                    : findsOneWidget,
              );
              switch (SizeClass.fromWidth(size.value.width)) {
                case SizeClass.compact:
                  expect(find.byType(NavigationDestination), findsNWidgets(5));
                  expect(find.byTooltip(l10n.navSettings), findsOneWidget);
                case SizeClass.medium:
                  final rail = tester.widget<NavigationRail>(
                    find.byType(NavigationRail),
                  );
                  expect(rail.destinations, hasLength(7));
                  expect(rail.selectedIndex, 0);
                case SizeClass.expanded:
                  expect(find.byType(SidebarItem), findsNWidgets(8));
                  expect(find.text(l10n.actionNewCapture), findsOneWidget);
              }
              expect(tester.takeException(), isNull);
              final handle = tester.ensureSemantics();
              await expectLater(
                tester,
                meetsGuideline(androidTapTargetGuideline),
              );
              await expectLater(
                tester,
                meetsGuideline(labeledTapTargetGuideline),
              );
              await expectLater(tester, meetsGuideline(textContrastGuideline));
              handle.dispose();
            });
          }
        }
      }
    }
  });

  group('navigation', () {
    testWidgets('bottom bar switches branches and keeps typed locations', (
      tester,
    ) async {
      await _boot(tester);
      final router = _router(tester);
      expect(router.state.uri.path, const HomeRoute().location);
      await tester.tap(find.text('Inbox'));
      await tester.pumpAndSettle();
      expect(find.byType(InboxScreen), findsOneWidget);
      expect(router.state.uri.path, '/inbox');
      await tester.tap(find.text('Ask'));
      await tester.pumpAndSettle();
      expect(find.byType(AskScreen), findsOneWidget);
      await tester.tap(find.byTooltip('Settings'));
      await tester.pumpAndSettle();
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(router.state.uri.path, '/settings');
    });

    testWidgets('rail reaches the medium-only destinations', (tester) async {
      await _boot(tester, size: StrataTestSizes.medium);
      await tester.tap(find.text('Map'));
      await tester.pumpAndSettle();
      expect(find.byType(MapScreen), findsOneWidget);
      await tester.tap(find.text('Tasks'));
      await tester.pumpAndSettle();
      expect(find.byType(TasksScreen), findsOneWidget);
    });

    testWidgets('deep links open nested and standalone routes', (tester) async {
      await _boot(tester, size: StrataTestSizes.expanded);
      final router = _router(tester)
        ..go(const NoteEditorRoute(noteId: 'pricing-01').location);
      await tester.pumpAndSettle();
      expect(find.byType(NoteEditorScreen), findsOneWidget);
      // Nested in the Notes branch: the sidebar highlights Notes.
      final notes = tester
          .widgetList<SidebarItem>(find.byType(SidebarItem))
          .firstWhere((i) => i.destination.label == 'Notes');
      expect(notes.selected, isTrue);

      router.go(const SyncRoute().location);
      await tester.pumpAndSettle();
      expect(find.byType(SyncScreen), findsOneWidget);
      expect(find.byType(AdaptiveScaffold), findsNothing);
      expect(find.text('Sync & conflicts'), findsWidgets);

      router.go('/does-not-exist');
      await tester.pumpAndSettle();
      expect(find.text('Page not found'), findsOneWidget);
    });

    testWidgets('branch state survives a resize across breakpoints', (
      tester,
    ) async {
      await _boot(tester);
      await tester.tap(find.text('Inbox'));
      await tester.pumpAndSettle();
      for (final size in [
        StrataTestSizes.medium,
        StrataTestSizes.expanded,
        StrataTestSizes.large,
        StrataTestSizes.compact,
      ]) {
        tester.view.physicalSize = size;
        await tester.pumpAndSettle();
        expect(find.byType(InboxScreen), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets('no sync pill until the core reports a status', (tester) async {
      await _boot(tester, sync: null);
      expect(find.byType(SyncPill), findsNothing);
    });
  });

  group('typed routes', () {
    test('locations', () {
      expect(const HomeRoute().location, '/home');
      expect(const InboxRoute().location, '/inbox');
      expect(const TasksRoute().location, '/tasks');
      expect(const NotesRoute().location, '/notes');
      expect(
        const NoteEditorRoute(noteId: 'Call 2026-09-12').location,
        '/notes/Call%202026-09-12',
      );
      expect(const MapRoute().location, '/map');
      expect(const DirectoryRoute().location, '/directory');
      expect(const DocumentsRoute().location, '/directory/documents');
      expect(const AskRoute().location, '/ask');
      expect(const SettingsRoute().location, '/settings');
      expect(const AdminUsersRoute().location, '/settings/admin/users');
      expect(const SyncRoute().location, '/sync');
      expect(const SignInRoute().location, '/sign-in');
    });

    test('shell branches follow the destination order', () {
      final shell = $appRoutes.first as StatefulShellRoute;
      expect(shell.branches, hasLength(AppDestination.values.length));
      final firstPaths = [
        for (final branch in shell.branches)
          (branch.routes.first as GoRoute).path,
      ];
      expect(firstPaths, [
        '/home',
        '/inbox',
        '/tasks',
        '/notes',
        '/map',
        '/directory',
        '/ask',
        '/settings',
      ]);
    });

    test('destinations mirror the spec per size class', () async {
      final l10n = await StrataLocalizations.delegate.load(
        StrataLocales.english,
      );
      final all = appDestinations(l10n);
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
  });
}
