import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/harness.dart';

final _petrolScreen = TaskScreen(
  id: 't-petrol-arrows',
  task: StrataFixtures.taskPetrolArrowsInvoice,
  line: '- [ ] Petrol Arrows invoice 🔁 every week on Sunday',
  history: const [],
);

const _empty = TasksView(
  sections: TaskSections(
    overdue: [],
    today: [],
    upcoming: [],
    recurring: [],
    noDate: [],
  ),
  done: [],
);

Finder _semantics(String label) => find.bySemanticsLabel(label);

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

void main() {
  group('TasksScreen content', () {
    for (final v in matrix()) {
      testWidgets('structure, intents and accessibility [$v]', (tester) async {
        final fake = FakeCoreApi()
          ..tasks.add(StrataFixtures.tasksView)
          ..task['t-watanya-eta'].add(StrataFixtures.taskScreen)
          ..task['t-petrol-arrows'].add(_petrolScreen);
        final opened = <String>[];
        await pumpVariant(
          tester,
          v,
          TasksScreen(
            initialTaskId: v.sizeClass == SizeClass.compact
                ? null
                : 't-watanya-eta',
            onOpenTask: opened.add,
          ),
          fake: fake,
        );
        final s = lookupTasksLocalizations(v.locale);
        expectNoErrors(tester);
        expect(fake.calls, contains(const CoreCall('watchTasks')));

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(StrataPanes), findsNothing);
            expect(find.text(s.tasksTitle), findsOneWidget);
            expect(
              find.text(s.tasksTabWithCount(label: s.tasksTabToday, count: 1)),
              findsOneWidget,
            );
            expect(find.text('Petrol Arrows invoice'), findsOneWidget);
            expect(find.byType(TaskDetailPane), findsNothing);
            await _tap(tester, find.text('Petrol Arrows invoice'));
            expect(opened, ['t-petrol-arrows']);
          case SizeClass.medium:
            expect(find.byType(StrataPanes), findsOneWidget);
            expect(
              find.descendant(
                of: find.byType(TaskDetailPane),
                matching: find.text("Make Watanya's ETA invoice"),
              ),
              findsOneWidget,
            );
            // J selects the first row of the Today view, X completes it.
            await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
            await tester.pump();
            await tester.sendKeyEvent(LogicalKeyboardKey.keyX);
            await tester.pump();
            expect(
              fake.calls,
              contains(
                const CoreCall('completeTask', {'taskId': 't-petrol-arrows'}),
              ),
            );
          case SizeClass.expanded:
            expect(find.byType(StrataPanes), findsNothing);
            expect(find.text(s.tasksColumnRepeat), findsWidgets);
            expect(find.text(s.tasksTabOverdue), findsOneWidget);
            expect(
              find.text('Pay Nile Freight September invoice'),
              findsOneWidget,
            );
            expect(find.byType(KeyboardHintChip), findsWidgets);
            expect(_semantics(s.tasksDetailPanelLabel), findsOneWidget);
            expect(
              find.descendant(
                of: find.byType(TaskDetailPane),
                matching: find.text(s.tasksFieldRepeat),
              ),
              findsOneWidget,
            );
        }

        await _tap(
          tester,
          _semantics(s.tasksMarkDoneSemantics(title: 'Petrol Arrows invoice')),
        );
        expect(
          fake.calls,
          contains(
            const CoreCall('completeTask', {'taskId': 't-petrol-arrows'}),
          ),
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
      });
    }
  });

  group('TasksScreen states', () {
    for (final v in matrix()) {
      testWidgets('empty [$v]', (tester) async {
        final fake = FakeCoreApi()..tasks.add(_empty);
        await pumpVariant(tester, v, const TasksScreen(), fake: fake);
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.tasksEmptyTitle), findsOneWidget);
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('overdue view [$v]', (tester) async {
        final fake = FakeCoreApi()..tasks.add(StrataFixtures.tasksView);
        await pumpVariant(
          tester,
          v,
          const TasksScreen(initialTab: TasksTab.overdue),
          fake: fake,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(
          find.text(s.tasksOverdueSince(date: DateTime.utc(2026, 9, 24))),
          findsOneWidget,
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('loading and error [$v]', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const TasksScreen(),
          fake: fake,
          settle: false,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(_semantics(s.commonLoading), findsOneWidget);
        fake.tasks.addError(StrataFixtures.coreFailure);
        await tester.pump();
        expect(find.text(s.tasksLoadError), findsOneWidget);
        expect(
          find.text(s.commonCoreFailure(code: 'pending_changes')),
          findsOneWidget,
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
      });
    }
  });

  group('TasksScreen interactions', () {
    testWidgets('compact pushes the task detail without a route callback', (
      tester,
    ) async {
      final v = matrix(sizes: {'compact': StrataTestSizes.compact}).first;
      final fake = FakeCoreApi()
        ..tasks.add(StrataFixtures.tasksView)
        ..task['t-petrol-arrows'].add(_petrolScreen);
      await pumpVariant(tester, v, const TasksScreen(), fake: fake);
      await _tap(tester, find.text('Petrol Arrows invoice'));
      expect(find.byType(TaskDetailScreen), findsOneWidget);
      expect(
        fake.calls,
        contains(const CoreCall('watchTask', {'id': 't-petrol-arrows'})),
      );
    });

    testWidgets('views map 1:1 to the core sections and done reopens', (
      tester,
    ) async {
      final v = matrix(sizes: {'compact': StrataTestSizes.compact}).first;
      final fake = FakeCoreApi()..tasks.add(StrataFixtures.tasksView);
      await pumpVariant(tester, v, const TasksScreen(), fake: fake);
      final s = lookupTasksLocalizations(v.locale);
      await _tap(tester, find.text(s.tasksTabDone));
      expect(
        find.text(s.tasksDoneOn(date: DateTime.utc(2026, 9))),
        findsOneWidget,
      );
      await _tap(
        tester,
        _semantics(s.tasksReopenSemantics(title: "Make Watanya's ETA invoice")),
      );
      expect(
        fake.calls,
        contains(
          const CoreCall('reopenTask', {'taskId': 't-watanya-eta-2026-09'}),
        ),
      );
      await _tap(
        tester,
        find.text(s.tasksTabWithCount(label: s.tasksTabRecurring, count: 2)),
      );
      expect(find.byType(RecurringRuleRow), findsNWidgets(2));
    });

    testWidgets('expanded keyboard: Enter, R and T', (tester) async {
      final v = matrix(sizes: {'expanded': StrataTestSizes.expanded}).first;
      final fake = FakeCoreApi()
        ..tasks.add(StrataFixtures.tasksView)
        ..task['t-watanya-eta'].add(StrataFixtures.taskScreen);
      final notes = <String>[];
      await pumpVariant(
        tester,
        v,
        TasksScreen(initialTaskId: 't-watanya-eta', onOpenNote: notes.add),
        fake: fake,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(notes, ['n-tasks']);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
      await tester.pumpAndSettle();
      expect(find.byType(RecurrenceEditor), findsOneWidget);
      expect(find.byType(Dialog), findsOneWidget);
      await tester.tapAt(const Offset(4, 4));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyT);
      await tester.pumpAndSettle();
      expect(find.byType(TaskEditorSheet), findsOneWidget);
    });
  });
}
