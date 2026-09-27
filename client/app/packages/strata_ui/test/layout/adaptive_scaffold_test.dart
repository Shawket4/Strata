import 'package:flutter/material.dart';
import 'dart:ui' show Tristate;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/harness.dart';

Future<StrataLocalizations> _l10n(MatrixVariant v) =>
    StrataLocalizations.delegate.load(v.locale);

void main() {
  group('AdaptiveScaffold structure', () {
    for (final v in variants()) {
      testWidgets('$v', (tester) async {
        await pumpVariant(tester, v, const TestShell());
        final l10n = await _l10n(v);

        expect(
          Directionality.of(tester.element(find.byType(AdaptiveScaffold))),
          v.direction,
        );
        expect(find.bySemanticsLabel(l10n.navigationLabel), findsOneWidget);

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(NavigationBar), findsOneWidget);
            expect(find.byType(NavigationRail), findsNothing);
            expect(find.byType(StrataSidebar), findsNothing);
            expect(find.byType(NavigationDestination), findsNWidgets(5));
            final bar = tester.widget<NavigationBar>(
              find.byType(NavigationBar),
            );
            expect(bar.selectedIndex, 1);
            for (final label in [
              l10n.navHome,
              l10n.navInbox,
              l10n.navNotes,
              l10n.navDirectory,
              l10n.navAsk,
            ]) {
              expect(
                find.descendant(
                  of: find.byType(NavigationBar),
                  matching: find.text(label),
                ),
                findsOneWidget,
              );
            }
            expect(find.text(l10n.navTasks), findsNothing);
            expect(find.text(l10n.navSettings), findsNothing);
            expect(find.byType(AppBar), findsOneWidget);
            expect(find.byTooltip(l10n.actionSearch), findsOneWidget);
            final pill = tester.widget<SyncPill>(find.byType(SyncPill));
            expect(pill.dense, isFalse);
            expect(
              find.text(l10n.syncOfflineQueued(count: 3)),
              findsOneWidget,
            );
          case SizeClass.medium:
            expect(find.byType(NavigationBar), findsNothing);
            expect(find.byType(StrataSidebar), findsNothing);
            final rail = tester.widget<NavigationRail>(
              find.byType(NavigationRail),
            );
            expect(rail.destinations, hasLength(7));
            expect(rail.selectedIndex, 1);
            expect(rail.scrollable, isTrue);
            expect(
              tester.getSize(find.byType(NavigationRail)).width,
              StrataLayout.railWidth,
            );
            expect(find.byType(RailFooterDestination), findsOneWidget);
            expect(find.byTooltip(l10n.actionNewCapture), findsOneWidget);
            expect(tester.widget<SyncPill>(find.byType(SyncPill)).dense, true);
            expect(find.byType(AppBar), findsNothing);
          case SizeClass.expanded:
            expect(find.byType(NavigationBar), findsNothing);
            expect(find.byType(NavigationRail), findsNothing);
            expect(
              tester.getSize(find.byType(StrataSidebar)).width,
              StrataLayout.sidebarWidth,
            );
            expect(find.byType(SidebarItem), findsNWidgets(8));
            expect(find.byType(StrataWordmark), findsOneWidget);
            expect(find.text(l10n.actionNewCapture), findsOneWidget);
            expect(find.byType(KeyboardHintChip), findsOneWidget);
            expect(
              find.bySemanticsLabel('${l10n.navInbox}, 4'),
              findsOneWidget,
            );
            expect(
              find.bySemanticsLabel('${l10n.navTasks}, 2'),
              findsOneWidget,
            );
            final inbox = semanticsOf(
              tester,
              find.bySemanticsLabel('${l10n.navInbox}, 4'),
            );
            expect(inbox.flagsCollection.isSelected, Tristate.isTrue);
            expect(find.byType(AppBar), findsNothing);
        }
        expectNoRenderErrors(tester);
      });
    }
  });

  group('AdaptiveScaffold accessibility', () {
    for (final v in variants()) {
      testWidgets('$v', (tester) async {
        await pumpVariant(tester, v, const TestShell());
        await expectAccessible(tester);
      });
    }
  });

  group('AdaptiveScaffold selection', () {
    final en = variants(textScales: const [1]).where(
      (v) =>
          v.brightness == Brightness.light && v.locale.languageCode == 'en',
    );
    final byClass = {for (final v in en) v.sizeClass: v};

    testWidgets('bottom bar maps taps to full destination indices', (
      tester,
    ) async {
      final selected = <int>[];
      await pumpVariant(
        tester,
        byClass[SizeClass.compact]!,
        TestShell(onSelected: selected.add),
      );
      await tester.tap(find.text('Notes'));
      await tester.tap(find.text('Directory'));
      await tester.tap(find.text('Ask'));
      expect(selected, [Dest.notes, Dest.directory, Dest.ask]);
    });

    testWidgets('destinations hidden on compact highlight their host', (
      tester,
    ) async {
      await pumpVariant(
        tester,
        byClass[SizeClass.compact]!,
        const TestShell(selectedIndex: Dest.tasks),
      );
      expect(
        tester.widget<NavigationBar>(find.byType(NavigationBar)).selectedIndex,
        0,
      );
      await pumpVariant(
        tester,
        byClass[SizeClass.compact]!,
        const TestShell(selectedIndex: Dest.map),
      );
      expect(
        tester.widget<NavigationBar>(find.byType(NavigationBar)).selectedIndex,
        2,
      );
    });

    testWidgets('rail maps taps and exposes Settings at the bottom', (
      tester,
    ) async {
      final selected = <int>[];
      await pumpVariant(
        tester,
        byClass[SizeClass.medium]!,
        TestShell(onSelected: selected.add, selectedIndex: Dest.settings),
      );
      expect(
        tester.widget<NavigationRail>(find.byType(NavigationRail)).selectedIndex,
        isNull,
      );
      final footer = tester.widget<RailFooterDestination>(
        find.byType(RailFooterDestination),
      );
      expect(footer.selected, isTrue);
      await tester.tap(find.text('Tasks'));
      await tester.tap(find.text('Map'));
      await tester.tap(find.text('Settings'));
      expect(selected, [Dest.tasks, Dest.map, Dest.settings]);
      final railBottom = tester.getBottomLeft(find.byType(NavigationRail)).dy;
      final settingsBottom = tester.getBottomLeft(find.text('Settings')).dy;
      expect(railBottom - settingsBottom, lessThan(120));
    });

    testWidgets('sidebar maps taps and capture triggers the action', (
      tester,
    ) async {
      final selected = <int>[];
      var captures = 0;
      await pumpVariant(
        tester,
        byClass[SizeClass.expanded]!,
        TestShell(onSelected: selected.add, onCapture: () => captures++),
      );
      await tester.tap(find.text('Map'));
      await tester.tap(find.text('Settings'));
      await tester.tap(find.text('New capture'));
      expect(selected, [Dest.map, Dest.settings]);
      expect(captures, 1);
    });

    testWidgets('Ctrl+N and Cmd+N trigger capture', (tester) async {
      var captures = 0;
      await pumpVariant(
        tester,
        byClass[SizeClass.expanded]!,
        TestShell(
          onCapture: () => captures++,
          body: const Focus(autofocus: true, child: SizedBox.expand()),
        ),
      );
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      expect(captures, 2);
    });
  });

  group('AdaptiveScaffold live resize', () {
    testWidgets('re-evaluates the navigation on every resize', (tester) async {
      final v = variants(textScales: const [1]).first;
      await pumpVariant(tester, v, const TestShell());
      Type nav() {
        if (find.byType(NavigationBar).evaluate().isNotEmpty) {
          return NavigationBar;
        }
        if (find.byType(NavigationRail).evaluate().isNotEmpty) {
          return NavigationRail;
        }
        return StrataSidebar;
      }

      final steps = <double, Type>{
        390: NavigationBar,
        599: NavigationBar,
        600: NavigationRail,
        1024: NavigationRail,
        1199: NavigationRail,
        1200: StrataSidebar,
        1920: StrataSidebar,
        480: NavigationBar,
      };
      for (final step in steps.entries) {
        tester.view.physicalSize = Size(step.key, 800);
        await tester.pumpAndSettle();
        expect(nav(), step.value, reason: 'width ${step.key}');
        expectNoRenderErrors(tester);
      }
    });
  });

  group('AdaptiveScaffold contract', () {
    testWidgets('rejects more than five compact destinations', (tester) async {
      setWindowSize(tester, StrataTestSizes.compact);
      await tester.pumpWidget(
        MaterialApp(
          localizationsDelegates: StrataLocalizations.localizationsDelegates,
          home: AdaptiveScaffold(
            destinations: [
              for (var i = 0; i < 6; i++)
                StrataDestination(icon: Icons.circle, label: 'D$i'),
            ],
            selectedIndex: 0,
            onDestinationSelected: (_) {},
            body: const SizedBox(),
            title: 'T',
            navigationLabel: 'Nav',
          ),
        ),
      );
      expect(tester.takeException(), isAssertionError);
    });
  });
}
