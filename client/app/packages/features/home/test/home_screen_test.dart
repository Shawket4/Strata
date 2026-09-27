import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_home/strata_home.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/harness.dart';

final _offline = HomeView(
  recentNotes: StrataFixtures.homeView.recentNotes,
  inboxCount: 3,
  tasks: StrataFixtures.taskSections,
  sync_: StrataFixtures.syncPillOffline,
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
  ),
  sync_: StrataFixtures.syncPill,
);

Future<void> _reveal(WidgetTester tester, Finder finder) async {
  final scrollables = find.byType(Scrollable).evaluate().toList();
  for (final scrollable in scrollables) {
    for (final step in const [-200.0, 200.0]) {
      for (var i = 0; i < 20 && finder.evaluate().isEmpty; i++) {
        await tester.drag(find.byWidget(scrollable.widget), Offset(0, step));
        await tester.pump();
      }
      if (finder.evaluate().isNotEmpty) return;
    }
  }
}

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _reveal(tester, finder);
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

Finder _composer() => find.byType(TextField);

void main() {
  group('HomeScreen content', () {
    for (final v in matrix()) {
      testWidgets('structure, intents and accessibility [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
        final notes = <String>[];
        var inbox = 0;
        await pumpVariant(
          tester,
          v,
          HomeScreen(onOpenNote: notes.add, onOpenInbox: () => inbox++),
          fake: fake,
        );
        final s = lookupHomeLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expectNoErrors(tester);
        expect(fake.calls, contains(const CoreCall('watchHome')));
        expect(find.byType(CaptureComposer), findsOneWidget);
        await expectAccessible(tester);

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(KeyboardHintChip), findsNothing);
            expect(find.text(s.homeTitle), findsNothing);
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
            expect(find.text(s.homeTitle), findsOneWidget);
            expect(find.byType(KeyboardHintChip), findsWidgets);
            expect(
              find.bySemanticsLabel('${t.tasksTabUpcoming}, 2'),
              findsNothing,
            );
            expect(find.byType(RecurringRuleRow), findsNothing);
            await _reveal(tester, find.text(s.homeAiActivity));
            expect(find.text(s.homeNotYetAvailable), findsWidgets);
        }

        await _tap(tester, find.text(s.homeInboxWaiting(count: 2)));
        expect(inbox, 1);
        await _tap(tester, find.text('Pricing experiments'));
        expect(notes, ['n-pricing-experiments']);
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
    for (final v in matrix()) {
      testWidgets('offline [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(_offline);
        await pumpVariant(tester, v, const HomeScreen(), fake: fake);
        final s = lookupHomeLocalizations(v.locale);
        expect(find.text(s.homeComposerOffline), findsOneWidget);
        await _reveal(tester, find.text(s.homeInboxWaiting(count: 3)));
        expect(find.text(s.homeInboxWaiting(count: 3)), findsOneWidget);
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('empty [$v]', (tester) async {
        final fake = FakeCoreApi()..home.add(_empty);
        await pumpVariant(tester, v, const HomeScreen(), fake: fake);
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
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('loading then error; the composer stays usable [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const HomeScreen(),
          fake: fake,
          settle: false,
        );
        final s = lookupHomeLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expect(find.byType(CaptureComposer), findsOneWidget);
        expect(find.bySemanticsLabel(t.commonLoading), findsOneWidget);
        fake.home.addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(find.text(s.homeLoadError), findsOneWidget);
        expect(find.byType(CaptureComposer), findsOneWidget);
        await expectAccessible(tester);
        expectNoErrors(tester);
      });
    }
  });

  group('HomeScreen keyboard and failures', () {
    testWidgets('Ctrl+N focuses the composer, Ctrl+Enter saves', (
      tester,
    ) async {
      final v = matrix().firstWhere((v) => v.sizeName == 'expanded');
      final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
      await pumpVariant(tester, v, const HomeScreen(), fake: fake);
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
      await tester.pumpAndSettle();
      expect(
        fake.calls.last,
        const CoreCall('capture', {'text': 'Idea: loyalty tier'}),
      );
    });

    testWidgets('a refused capture keeps the text and says why', (
      tester,
    ) async {
      final v = matrix().first;
      final fake = FakeCoreApi()
        ..home.add(StrataFixtures.homeView)
        ..captureAnswer.throws(StrataFixtures.coreFailure);
      await pumpVariant(tester, v, const HomeScreen(), fake: fake);
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
