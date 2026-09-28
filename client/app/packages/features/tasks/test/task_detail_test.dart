import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';

import 'helpers/harness.dart';

Future<void> _tap(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isEmpty) {
    await tester.scrollUntilVisible(
      finder,
      200,
      scrollable: find.byType(Scrollable).first,
    );
  }
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

void main() {
  group('TaskDetailScreen', () {
    for (final v in matrix()) {
      testWidgets('recurring task: fields, history and intents [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..task['t-watanya-eta'].add(StrataFixtures.taskScreen);
        final notes = <String>[];
        await pumpVariant(
          tester,
          v,
          TaskDetailScreen(taskId: 't-watanya-eta', onOpenNote: notes.add),
          fake: fake,
          scaffold: false,
        );
        final s = lookupTasksLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(const CoreCall('watchTask', {'id': 't-watanya-eta'})),
        );
        expect(find.text(s.tasksDetailTitle), findsOneWidget);
        expect(find.text("Make Watanya's ETA invoice"), findsOneWidget);
        expect(find.text('every month on the 1st'), findsOneWidget);
        expect(find.text(s.tasksRecurring), findsOneWidget);
        await expectAccessible(tester);
        final scrollable = find.byType(Scrollable).first;
        await tester.scrollUntilVisible(
          find.byType(TaskReminderBell),
          200,
          scrollable: scrollable,
        );
        await tester.scrollUntilVisible(
          find.text(s.tasksDoneOn(date: DateTime.utc(2026, 9))),
          200,
          scrollable: scrollable,
        );
        await tester.scrollUntilVisible(
          find.text(s.tasksNoSkipping),
          200,
          scrollable: scrollable,
        );

        await _tap(tester, find.text(s.tasksActionMarkDone));
        expect(
          fake.calls,
          contains(const CoreCall('completeTask', {'taskId': 't-watanya-eta'})),
        );
        await _tap(tester, find.text(s.tasksActionCancelTask));
        expect(
          fake.calls,
          contains(const CoreCall('cancelTask', {'taskId': 't-watanya-eta'})),
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('not found [$v]', (tester) async {
        final fake = FakeCoreApi()
          ..task['t-gone'].add(
            const TaskScreen(
              id: 't-gone',
              line: '',
              history: [],
              locationLabel: '',
              recurrencePreview: [],
            ),
          );
        await pumpVariant(
          tester,
          v,
          const TaskDetailScreen(taskId: 't-gone'),
          fake: fake,
          scaffold: false,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.tasksNotFoundTitle), findsOneWidget);
        await expectAccessible(tester);
        expectNoErrors(tester);
      });

      testWidgets('done one-off task offers Reopen [$v]', (tester) async {
        final fake = FakeCoreApi()
          ..task['t-watanya-eta-2026-09'].add(
            TaskScreen(
              id: 't-watanya-eta-2026-09',
              task: StrataFixtures.taskWatanyaDoneSeptember,
              line: "- [x] Make Watanya's ETA invoice ✅ 2026-09-01",
              history: const [],
              locationLabel: '',
              recurrencePreview: [],
            ),
          );
        await pumpVariant(
          tester,
          v,
          const TaskDetailScreen(taskId: 't-watanya-eta-2026-09'),
          fake: fake,
          scaffold: false,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.tasksActionMarkDone), findsNothing);
        await _tap(tester, find.text(s.tasksActionReopen));
        expect(
          fake.calls,
          contains(
            const CoreCall('reopenTask', {'taskId': 't-watanya-eta-2026-09'}),
          ),
        );
        await expectAccessible(tester);
        expectNoErrors(tester);
      });
    }
  });

  group('Task detail editing', () {
    final compact = matrix().first;

    testWidgets('Edit rule saves the phrase verbatim; Stop repeating clears', (
      tester,
    ) async {
      final fake = FakeCoreApi()
        ..task['t-watanya-eta'].add(StrataFixtures.taskScreen);
      await pumpVariant(
        tester,
        compact,
        const TaskDetailScreen(taskId: 't-watanya-eta'),
        fake: fake,
        scaffold: false,
      );
      final s = lookupTasksLocalizations(compact.locale);
      await _tap(tester, find.text(s.tasksActionEditRule));
      expect(find.byType(RecurrenceEditor), findsOneWidget);
      expect(find.byType(BottomSheet), findsOneWidget);
      expect(find.text(s.recurrencePreviewUnavailable), findsOneWidget);
      await tester.enterText(
        find.byType(TextField),
        'every month on the last day',
      );
      await _tap(tester, find.text(s.recurrenceSave));
      expect(
        fake.calls,
        contains(
          const CoreCall('updateTask', {
            'taskId': 't-watanya-eta',
            'patch': TaskPatch(
              recurrence: 'every month on the last day',
              clearDue: false,
              clearRecurrence: false,
            ),
          }),
        ),
      );
      await _tap(tester, find.text(s.tasksActionEditRule));
      await _tap(tester, find.text(s.recurrenceStop));
      expect(
        fake.calls,
        contains(
          const CoreCall('updateTask', {
            'taskId': 't-watanya-eta',
            'patch': TaskPatch(clearDue: false, clearRecurrence: true),
          }),
        ),
      );
    });

    testWidgets('Edit text and remove due date', (tester) async {
      final fake = FakeCoreApi()
        ..task['t-watanya-eta'].add(StrataFixtures.taskScreen);
      await pumpVariant(
        tester,
        compact,
        const TaskDetailScreen(taskId: 't-watanya-eta'),
        fake: fake,
        scaffold: false,
      );
      final s = lookupTasksLocalizations(compact.locale);
      await _tap(tester, find.byTooltip(s.tasksActionEditText));
      await tester.enterText(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.byType(TextField),
        ),
        "Make Watanya's e-invoice",
      );
      await _tap(tester, find.text(s.commonSave));
      expect(
        fake.calls,
        contains(
          const CoreCall('updateTask', {
            'taskId': 't-watanya-eta',
            'patch': TaskPatch(
              text: "Make Watanya's e-invoice",
              clearDue: false,
              clearRecurrence: false,
            ),
          }),
        ),
      );
      await _tap(tester, find.byTooltip(s.tasksActionClearDue));
      expect(
        fake.calls,
        contains(
          const CoreCall('updateTask', {
            'taskId': 't-watanya-eta',
            'patch': TaskPatch(clearDue: true, clearRecurrence: false),
          }),
        ),
      );
    });

    testWidgets('change due date through the picker', (tester) async {
      final fake = FakeCoreApi()
        ..task['t-watanya-eta'].add(StrataFixtures.taskScreen);
      await pumpVariant(
        tester,
        compact,
        const TaskDetailScreen(taskId: 't-watanya-eta'),
        fake: fake,
        scaffold: false,
      );
      final s = lookupTasksLocalizations(compact.locale);
      await _tap(tester, find.byTooltip(s.tasksActionChangeDue));
      await _tap(tester, find.text('15'));
      await _tap(tester, find.text('OK'));
      expect(
        fake.calls,
        contains(
          CoreCall('updateTask', {
            'taskId': 't-watanya-eta',
            'patch': TaskPatch(
              due: DateTime.utc(2026, 10, 15),
              clearDue: false,
              clearRecurrence: false,
            ),
          }),
        ),
      );
    });
  });
}
