import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

final _offline = HomeView(
  recentNotes: StrataFixtures.homeView.recentNotes,
  inboxCount: 3,
  tasks: StrataFixtures.taskSections,
  sync_: StrataFixtures.syncPillOffline,
  todayLabel: '',
  greeting: '',
  displayName: '',
  inboxPreview: [],
  needsYouCount: 0,
  contradictionsCount: 0,
  inboxSummary: '',
  aiActivity: Availability.available,
  aiActivityItems: [],
  aiActivityHeadline: '',
  openItems: Availability.available,
  openItemList: [],
  pinned: [],
);

final _empty = HomeView(
  recentNotes: const [],
  inboxCount: 0,
  tasks: const TaskSections(
    overdue: [],
    today: [],
    upcoming: [],
    recurring: [],
    noDate: [],
    upcomingGroups: [],
    todayCount: 0,
  ),
  sync_: StrataFixtures.syncPill,
  todayLabel: '',
  greeting: '',
  displayName: '',
  inboxPreview: [],
  needsYouCount: 0,
  contradictionsCount: 0,
  inboxSummary: '',
  aiActivity: Availability.available,
  aiActivityItems: [],
  aiActivityHeadline: '',
  openItems: Availability.available,
  openItemList: [],
  pinned: [],
);

/// The page's vertical lists (not the text fields' own scrollables).
final Finder _lists = find.byWidgetPredicate(
  (w) =>
      w is Scrollable &&
      w.restorationId != 'editable' &&
      axisDirectionToAxis(w.axisDirection) == Axis.vertical,
);

/// Scrolls whichever list holds [finder] until it is built.
Future<void> _reveal(WidgetTester tester, Finder finder) async {
  final count = _lists.evaluate().length;
  for (var i = 0; i < count && finder.evaluate().isEmpty; i++) {
    final list = _lists.at(i);
    tester.state<ScrollableState>(list).position.jumpTo(0);
    await tester.pump();
    try {
      await tester.scrollUntilVisible(
        finder,
        150,
        scrollable: list,
        maxScrolls: 80,
      );
      // `scrollUntilVisible` reports "not in this list" as a StateError.
      // ignore: avoid_catching_errors
    } on StateError {
      continue;
    }
  }
}

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _reveal(tester, finder);
  await tapVisible(tester, finder.first);
}

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  Widget screen,
  FakeCoreApi fake,
) => pumpVariant(tester, v, screen, fake: fake, scaffold: true);

Finder _composer() => find.byType(TextField);

void main() {
  group('HomeScreen content', () {
    for (final v in variants()) {
      testWidgets('structure, intents and accessibility [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
        final notes = <(String, String?)>[];
        var inbox = 0;
        await _pump(
          tester,
          v,
          HomeScreen(
            onOpenNote: (id, anchor) => notes.add((id, anchor)),
            onOpenInbox: () => inbox++,
          ),
          fake,
        );
        final s = lookupHomeLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          containsAll(const [
            CoreCall('watchHome'),
            CoreCall('refreshAiActivity'),
          ]),
        );
        expect(find.byType(CaptureComposer), findsOneWidget);
        expect(
          directionOf(tester, find.byType(CaptureComposer)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        // The core's date and greeting.
        expect(find.text('Sunday, 27 September'), findsOneWidget);
        expect(find.text('Good morning, Shawket'), findsOneWidget);
        expect(find.text(s.homeTitle), findsNothing);
        await expectAccessible(tester, contrast: v.textScale == 1);

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(KeyboardHintChip), findsNothing);
            for (final (title, count) in [
              (t.tasksTabOverdue, 1),
              (t.tasksTabToday, 1),
              (t.tasksTabUpcoming, 2),
              (t.tasksTabRecurring, 2),
            ]) {
              final header = find.bySemanticsLabel('$title, $count');
              await _reveal(tester, header);
              expect(header, findsOneWidget);
            }
            expect(find.byType(RecurringRuleRow), findsWidgets);
          case SizeClass.medium:
          case SizeClass.expanded:
            expect(find.byType(KeyboardHintChip), findsWidgets);
            expect(
              find.bySemanticsLabel('${t.tasksTabUpcoming}, 2'),
              findsNothing,
            );
            expect(find.byType(RecurringRuleRow), findsNothing);
        }
        // The inbox preview, the AI activity and the open items.
        await _reveal(tester, find.text('1 ready to accept · 1 needs you'));
        expect(find.text(s.homeNeedsYou(count: 1)), findsOneWidget);
        expect(find.text('Who is “بابا”?'), findsOneWidget);
        await _reveal(tester, find.text('2 AI changes since yesterday'));
        expect(
          find.text('Ahmed Samir works at Acme Logistics'),
          findsOneWidget,
        );
        expect(find.text(s.homeAiUndone), findsOneWidget);
        await _reveal(tester, find.text('Send the weekly invoicing proposal'));
        expect(find.text(s.homeNotYetAvailable), findsNothing);

        await _tap(tester, find.text(s.homeInboxWaiting(count: 2)));
        expect(inbox, 1);
        await _tap(tester, find.text('Pricing experiments'));
        expect(notes, [('n-pricing-experiments', null)]);
        await _tap(
          tester,
          find.byWidgetPredicate(
            (w) =>
                w is Checkbox &&
                w.semanticLabel ==
                    t.tasksMarkDoneSemantics(
                      title: 'Pay Nile Freight September invoice',
                    ),
          ),
        );
        expect(
          fake.calls,
          contains(
            const CoreCall('completeTask', {'taskId': 't-nile-freight'}),
          ),
        );

        await _reveal(tester, _composer());
        await tester.ensureVisible(_composer());
        await tester.enterText(_composer(), 'بابا عايز يشوف الأرقام بكرة');
        await tester.pump();
        await _tap(tester, find.text(s.homeSave));
        expect(
          fake.calls.last,
          const CoreCall('capture', {'text': 'بابا عايز يشوف الأرقام بكرة'}),
        );
        expect(find.text(s.homeCaptureSaved), findsOneWidget);
        expect(tester.widget<TextField>(_composer()).controller!.text, isEmpty);
        expectNoErrors(tester);
      });
    }
  });

  group('HomeScreen states', () {
    for (final v in variants()) {
      testWidgets('offline [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(_offline);
        await _pump(tester, v, const HomeScreen(), fake);
        final s = lookupHomeLocalizations(v.locale);
        expect(find.text(s.homeComposerOffline), findsOneWidget);
        await _reveal(tester, find.text(s.homeInboxWaiting(count: 3)));
        expect(find.text(s.homeInboxWaiting(count: 3)), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
      });

      testWidgets('empty [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(_empty);
        await _pump(tester, v, const HomeScreen(), fake);
        final s = lookupHomeLocalizations(v.locale);
        await _reveal(tester, find.text(s.homeInboxWaiting(count: 0)));
        expect(find.text(s.homeInboxWaiting(count: 0)), findsOneWidget);
        final empty = v.sizeClass == SizeClass.compact
            ? s.homeNoOpenTasks
            : s.homeNothingToday;
        await _reveal(tester, find.text(empty));
        expect(find.text(empty), findsOneWidget);
        await _reveal(tester, find.text(s.homeNoNotes));
        expect(find.text(s.homeNoNotes), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
      });

      testWidgets('loading then error; the composer stays usable [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi();
        await _pump(tester, v, const HomeScreen(), fake);
        final s = lookupHomeLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expect(find.byType(CaptureComposer), findsOneWidget);
        expect(find.bySemanticsLabel(t.commonLoading), findsOneWidget);
        fake.home.addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(find.text(s.homeLoadError), findsOneWidget);
        expect(find.byType(CaptureComposer), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
      });
    }
  });

  group('HomeScreen keyboard and failures', () {
    testWidgets('Ctrl+N focuses the composer, Ctrl+Enter saves', (
      tester,
    ) async {
      final v = variants().firstWhere((v) => v.sizeName == 'expanded');
      final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
      await _pump(tester, v, const HomeScreen(), fake);
      EditableTextState editable() =>
          tester.state<EditableTextState>(find.byType(EditableText));
      expect(editable().widget.focusNode.hasFocus, isFalse);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(editable().widget.focusNode.hasFocus, isTrue);
      await tester.enterText(_composer(), 'Idea: loyalty tier');
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('capture', {'text': 'Idea: loyalty tier'}),
      );
    });

    testWidgets('an AI decision is undone or retyped', (tester) async {
      final v = variants().firstWhere((v) => v.sizeName == 'expanded');
      final fake = FakeCoreApi()
        ..home.add(StrataFixtures.homeView)
        ..relationTypesAnswer.returns(const [
          RelationTypeItem(key: 'works-at', label: 'works at'),
          RelationTypeItem(key: 'related', label: 'related'),
        ]);
      await _pump(tester, v, const HomeScreen(), fake);
      await _tap(tester, find.text('Undo'));
      expect(
        fake.calls.last,
        const CoreCall('rejectAiDecision', {'decisionId': 'dec-works-at'}),
      );
      await _tap(tester, find.text('Change type'));
      await tester.tap(find.text('related'));
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('retypeAiDecision', {
          'decisionId': 'dec-works-at',
          'relType': 'related',
        }),
      );
    });

    testWidgets('open items open their person and source', (tester) async {
      final v = variants().firstWhere((v) => v.sizeName == 'expanded');
      final notes = <(String, String?)>[];
      await _pump(
        tester,
        v,
        HomeScreen(onOpenNote: (id, anchor) => notes.add((id, anchor))),
        FakeCoreApi()..home.add(StrataFixtures.homeView),
      );
      await _reveal(tester, find.text('Send the weekly invoicing proposal'));
      final check = tester.widget<Checkbox>(
        find.descendant(
          of: find
              .ancestor(
                of: find.text('Send the weekly invoicing proposal'),
                matching: find.byType(Row),
              )
              .first,
          matching: find.byType(Checkbox),
        ),
      );
      expect(check.onChanged, isNull);
      await _tap(tester, find.widgetWithText(TextButton, 'Ahmed Samir'));
      await _tap(
        tester,
        find.bySemanticsLabel(
          v.l10n.citationSemantics(label: 'Call 2026-09-12 — Acme'),
        ),
      );
      expect(notes, [
        ('p-ahmed-samir', null),
        ('n-call-2026-09-12-acme', 'a1b2'),
      ]);
    });

    testWidgets('recent notes by filter come from the core', (tester) async {
      final v = variants().first;
      final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
      fake.recent[RecentFilter.filedByAi].add(
        RecentNotesView(
          filter: RecentFilter.filedByAi,
          notes: [StrataFixtures.noteListItemArabic],
        ),
      );
      await _pump(tester, v, const HomeScreen(), fake);
      await _tap(tester, find.text('Filed by AI'));
      expect(
        fake.calls,
        contains(
          const CoreCall('watchRecent', {'filter': RecentFilter.filedByAi}),
        ),
      );
      expect(find.text('تجارب التسعير — ملخص'), findsOneWidget);
      expect(find.text('Pricing experiments'), findsNothing);
    });

    testWidgets('AI activity offline says so', (tester) async {
      final v = variants().first;
      final home = StrataFixtures.homeView;
      await _pump(
        tester,
        v,
        const HomeScreen(),
        FakeCoreApi()
          ..home.add(
            HomeView(
              recentNotes: home.recentNotes,
              inboxCount: home.inboxCount,
              tasks: home.tasks,
              sync_: StrataFixtures.syncPillOffline,
              todayLabel: home.todayLabel,
              greeting: home.greeting,
              displayName: home.displayName,
              inboxPreview: home.inboxPreview,
              needsYouCount: home.needsYouCount,
              contradictionsCount: home.contradictionsCount,
              inboxSummary: home.inboxSummary,
              aiActivity: Availability.offline,
              aiActivityItems: const [],
              aiActivityHeadline: '',
              openItems: Availability.available,
              openItemList: const [],
              pinned: const [],
            ),
          ),
      );
      await _reveal(tester, find.text('Offline'));
      expect(find.text('Offline'), findsOneWidget);
      await _reveal(tester, find.text('No open items.'));
      expect(find.text('No open items.'), findsOneWidget);
    });

    testWidgets('a refused capture keeps the text and says why', (
      tester,
    ) async {
      final v = variants().first;
      final fake = FakeCoreApi()
        ..home.add(StrataFixtures.homeView)
        ..captureAnswer.throws(StrataFixtures.coreFailure);
      await _pump(tester, v, const HomeScreen(), fake);
      final s = lookupHomeLocalizations(v.locale);
      await tester.enterText(_composer(), 'Mona said rates go up 8%');
      await tester.pump();
      await _tap(tester, find.text(s.homeSave));
      expect(
        find.text(s.homeCaptureFailedCode(code: 'pending_changes')),
        findsOneWidget,
      );
      expect(
        tester.widget<TextField>(_composer()).controller!.text,
        'Mona said rates go up 8%',
      );
    });
  });
}
