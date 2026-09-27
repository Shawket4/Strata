import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';

import 'helpers/harness.dart';

const _opId = '01J8ZQ5N7P9R1T3V5X7Z9B1D3F';

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

void main() {
  group('DuplicatePromptSheet', () {
    for (final v in matrix()) {
      testWidgets('prompt, candidates and resolveDuplicate [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..duplicatePrompts.add(StrataFixtures.duplicatePromptsView);
        final opened = <CandidateItem>[];
        await pumpVariant(
          tester,
          v,
          SingleChildScrollView(
            child: DuplicatePromptSheet(onOpenExisting: opened.add),
          ),
          fake: fake,
        );
        final s = lookupTasksLocalizations(v.locale);
        expect(find.text(s.dupTitle), findsOneWidget);
        expect(
          find.text(
            s.dupSubtitle(
              kind: s.kindTask,
              title: "remind me to make watanya's invoice",
            ),
          ),
          findsOneWidget,
        );
        expect(find.text("Make Watanya's ETA invoice"), findsOneWidget);
        expect(
          find.text('monthly on the 1st · next Thu 1 Oct'),
          findsOneWidget,
        );
        await expectAccessible(tester);
        expectNoErrors(tester);

        await _tap(tester, find.text(s.dupCreateAnyway));
        expect(
          fake.calls.last,
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.createAnyway,
          }),
        );
        await _tap(tester, find.text(s.dupOpenExisting));
        expect(
          fake.calls.last,
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.discard,
          }),
        );
        expect(opened, [StrataFixtures.candidateItem]);
        await _tap(tester, find.text(s.dupCancel));
        expect(fake.calls.where((c) => c.method == 'resolveDuplicate'), [
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.createAnyway,
          }),
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.discard,
          }),
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.discard,
          }),
        ]);
      });
    }

    testWidgets('no open prompt', (tester) async {
      final fake = FakeCoreApi()
        ..duplicatePrompts.add(const DuplicatePromptsView(prompts: []));
      await pumpVariant(
        tester,
        matrix().first,
        const DuplicatePromptSheet(),
        fake: fake,
      );
      final s = lookupTasksLocalizations(matrix().first.locale);
      expect(find.text(s.dupNoneOpen), findsOneWidget);
    });

    testWidgets('show(): compact bottom sheet, expanded dialog; closes after '
        'an answer', (tester) async {
      for (final v in [matrix().first, matrix().last]) {
        await tester.pumpWidget(const SizedBox());
        final fake = FakeCoreApi()
          ..duplicatePrompts.add(StrataFixtures.duplicatePromptsView);
        await pumpVariant(
          tester,
          v,
          Builder(
            builder: (context) => TextButton(
              onPressed: () => DuplicatePromptSheet.show(context),
              child: const Text('open'),
            ),
          ),
          fake: fake,
        );
        await _tap(tester, find.text('open'));
        expect(
          find.byType(v.sizeName == 'compact' ? BottomSheet : Dialog),
          findsOneWidget,
        );
        final s = lookupTasksLocalizations(v.locale);
        await _tap(tester, find.text(s.dupCancel));
        expect(find.byType(DuplicatePromptSheet), findsNothing);
        expect(
          fake.calls.last,
          const CoreCall('resolveDuplicate', {
            'opId': _opId,
            'choice': DuplicateChoice.discard,
          }),
        );
      }
    });
  });
}
