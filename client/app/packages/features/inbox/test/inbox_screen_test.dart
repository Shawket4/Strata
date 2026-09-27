import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/harness.dart';

const _acme =
    'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري ابتداءً من '
    'أكتوبر';

Future<void> _tap(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isEmpty) {
    await tester.scrollUntilVisible(
      finder,
      200,
      scrollable: find.byType(Scrollable).last,
    );
  }
  await tester.ensureVisible(finder.first);
  await tester.pumpAndSettle();
  await tester.tap(finder.first);
  await tester.pumpAndSettle();
}

/// Scrolls [finder] into view when the list has not built it yet.
Future<void> _reveal(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isNotEmpty) return;
  await tester.scrollUntilVisible(
    finder,
    200,
    scrollable: find.byType(Scrollable).last,
  );
}

/// The one recorded call of [method].
CoreCall _only(FakeCoreApi fake, String method) =>
    fake.calls.singleWhere((c) => c.method == method);

/// [label] inside the card of type [T] that shows [text].
Finder _inCard<T>(String text, String label) => find.descendant(
  of: find.ancestor(of: find.text(text), matching: find.byType(T)).first,
  matching: find.text(label),
);

void main() {
  group('InboxScreen content', () {
    for (final v in matrix()) {
      testWidgets('structure, intents and accessibility [$v]', (tester) async {
        final fake = FakeCoreApi()..inbox.add(InboxFixtures.full);
        await pumpVariant(tester, v, const InboxScreen(), fake: fake);
        final s = lookupInboxLocalizations(v.locale);
        expectNoErrors(tester);
        expect(fake.calls, contains(const CoreCall('watchInbox')));
        await expectAccessible(tester);

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(StrataPanes), findsNothing);
            expect(find.text(s.inboxWhoIs(mention: 'بابا')), findsOneWidget);
            await _reveal(tester, find.text('Weekly invoicing request — Acme'));
            await _tap(tester, _inCard<CaptureCard>(_acme, s.inboxAccept));
            expect(
              fake.calls,
              contains(
                const CoreCall('acceptSuggestion', {'id': 's-filing-acme'}),
              ),
            );
          case SizeClass.medium:
            expect(find.byType(StrataPanes), findsOneWidget);
            expect(
              find.bySemanticsLabel('${s.inboxCapturesHeader}, 3'),
              findsOneWidget,
            );
            // The first capture is shown in the detail pane.
            await _reveal(tester, find.text('Weekly invoicing request — Acme'));
            await _tap(tester, find.text(s.inboxReject));
            expect(
              fake.calls,
              contains(
                const CoreCall('rejectSuggestion', {'id': 's-filing-acme'}),
              ),
            );
          case SizeClass.expanded:
            expect(find.byType(StrataPanes), findsNothing);
            final bulkBar = find.bySemanticsLabel(
              RegExp('^${s.inboxBulkActions}'),
            );
            expect(bulkBar, findsOneWidget);
            expect(find.text(s.inboxAlsoNeedsYou), findsOneWidget);
            await _tap(tester, find.byType(Checkbox).first);
            expect(find.text(s.inboxSelectedCount(count: 3)), findsOneWidget);
            await _tap(
              tester,
              find.descendant(of: bulkBar, matching: find.text(s.inboxAccept)),
            );
            expect(
              fake.calls.where((c) => c.method == 'acceptSuggestion').toList(),
              const [
                CoreCall('acceptSuggestion', {'id': 's-filing-acme'}),
                CoreCall('acceptSuggestion', {'id': 's-filing-loyalty'}),
                CoreCall('acceptSuggestion', {'id': 's-rel-contradicts'}),
                CoreCall('acceptSuggestion', {'id': 's-rel-part-of'}),
              ],
            );
            expect(find.text(s.inboxSelectedCount(count: 0)), findsOneWidget);
            if (v.textScale == 1) {
              await tester.scrollUntilVisible(
                find.byType(KeyboardHintChip).first,
                200,
                scrollable: find.byType(Scrollable).first,
              );
              expect(find.byType(KeyboardHintChip), findsWidgets);
            }
        }
        expectNoErrors(tester);
      });

      testWidgets('contradicts relation chip with AI confidence [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()..inbox.add(InboxFixtures.full);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(initialNoteId: 'n-capture-loyalty'),
          fake: fake,
        );
        final strata = lookupStrataLocalizationsFor(v);
        final chip = find.bySemanticsLabel(
          RegExp('^${strata.relationContradicts}: Discount policy'),
        );
        if (chip.evaluate().isEmpty) {
          await tester.scrollUntilVisible(
            chip,
            200,
            scrollable: find.byType(Scrollable).last,
          );
        }
        expect(chip, findsOneWidget);
        expect(
          find.text(RelationChip.aiTagText(strata.aiTag, 0.91)),
          findsOneWidget,
        );
        expectNoErrors(tester);
      });
    }
  });

  group('InboxScreen states', () {
    for (final v in matrix()) {
      testWidgets('link-or-create: create person, alias, reject [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()..inbox.add(InboxFixtures.linkOrCreate);
        await pumpVariant(tester, v, const InboxScreen(), fake: fake);
        final s = lookupInboxLocalizations(v.locale);
        expect(find.text(s.inboxWhoIs(mention: 'بابا')), findsWidgets);
        expect(find.text(s.inboxNoPersonMatches), findsWidgets);
        expect(find.text(s.inboxAliasNote(mention: 'بابا')), findsWidgets);
        await expectAccessible(tester);
        expectNoErrors(tester);

        await _tap(tester, find.text(s.inboxCreatePerson));
        expect(find.byType(CreatePersonSheet), findsOneWidget);
        await tester.enterText(
          find.descendant(
            of: find.byType(CreatePersonSheet),
            matching: find.byType(TextField),
          ),
          'Ibrahim Shawket',
        );
        await tester.pump();
        await _tap(tester, find.text(s.inboxCreatePersonSave));
        expect(_only(fake, 'createEntity').args, {
          'kind': 'person',
          'name': 'Ibrahim Shawket',
          'aliases': ['بابا'],
          'force': false,
        });
        expect(
          fake.calls.last,
          const CoreCall('acceptSuggestion', {'id': 's-who-is-baba'}),
        );
        expect(find.byType(CreatePersonSheet), findsNothing);
        // (On expanded the first "Reject" is the idle bulk bar's.)
        await _tap(tester, find.text(s.inboxReject).last);
        expect(
          fake.calls.last,
          const CoreCall('rejectSuggestion', {'id': 's-who-is-baba'}),
        );
      });

      testWidgets('custody: applied with Undo, ambiguous choice [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()..inbox.add(InboxFixtures.custody);
        await pumpVariant(tester, v, const InboxScreen(), fake: fake);
        final s = lookupInboxLocalizations(v.locale);
        expect(find.text(s.inboxAppliedAutomatically), findsWidgets);
        await expectAccessible(tester);
        expectNoErrors(tester);
        if (v.sizeClass == SizeClass.compact) {
          expect(find.text(s.inboxAiConfidence(score: '0.93')), findsOneWidget);
          expect(find.text(s.inboxWhichDocument), findsOneWidget);
          expect(
            find.text('Petrol Arrows commercial register'),
            findsOneWidget,
          );
          final accept = find.ancestor(
            of: find.text(s.inboxAccept),
            matching: find.byType(FilledButton),
          );
          expect(tester.widget<FilledButton>(accept).onPressed, isNull);
        }
        await _tap(tester, find.text(s.inboxUndo));
        expect(
          fake.calls.last,
          const CoreCall('rejectSuggestion', {'id': 's-custody-applied'}),
        );
      });

      testWidgets('duplicate-flagged, task and unsupported [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()..inbox.add(InboxFixtures.others);
        final opened = <String>[];
        await pumpVariant(
          tester,
          v,
          InboxScreen(onOpenNote: opened.add),
          fake: fake,
        );
        final s = lookupInboxLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expect(find.text(s.inboxPossibleDuplicate), findsWidgets);
        await expectAccessible(tester);
        expectNoErrors(tester);
        await _tap(tester, find.text(t.dupCreateAnyway));
        expect(
          fake.calls.last,
          const CoreCall('rejectSuggestion', {'id': 's-duplicate-watanya'}),
        );
        await _tap(tester, find.text(s.inboxDiscard));
        expect(
          fake.calls.last,
          const CoreCall('acceptSuggestion', {'id': 's-duplicate-watanya'}),
        );
        await _tap(tester, find.text(t.dupOpenExisting));
        expect(opened, ['t-watanya-eta']);
        if (v.sizeClass == SizeClass.compact) {
          expect(
            find.text("Make Watanya's ETA invoice · every month on the 1st"),
            findsOneWidget,
          );
          await _tap(
            tester,
            find.text(s.inboxUnsupported(kind: 'entity-merge')),
          );
          expect(find.text(s.inboxDismiss), findsOneWidget);
        }
      });

      testWidgets('empty, loading and error [$v]', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          settle: false,
        );
        final t = lookupTasksLocalizations(v.locale);
        final s = lookupInboxLocalizations(v.locale);
        expect(find.bySemanticsLabel(t.commonLoading), findsOneWidget);
        fake.inbox.add(InboxFixtures.empty);
        await tester.pump();
        await tester.pump();
        expect(find.text(s.inboxEmptyTitle), findsOneWidget);
        await expectAccessible(tester);
        expectNoErrors(tester);
      });
    }
  });

  group('InboxScreen keyboard and errors', () {
    testWidgets('expanded: J / X / A / R / E', (tester) async {
      final v = matrix().firstWhere((v) => v.sizeName == 'expanded');
      final fake = FakeCoreApi()..inbox.add(InboxFixtures.full);
      final opened = <String>[];
      await pumpVariant(
        tester,
        v,
        InboxScreen(onOpenNote: opened.add),
        fake: fake,
      );
      final s = lookupInboxLocalizations(v.locale);
      // Starts on the first capture; J moves to the loyalty capture.
      await tester.sendKeyEvent(LogicalKeyboardKey.keyJ);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyX);
      await tester.pump();
      expect(find.text(s.inboxSelectedCount(count: 1)), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyR);
      await tester.pump();
      expect(
        fake.calls.where((c) => c.method == 'rejectSuggestion').toList(),
        const [
          CoreCall('rejectSuggestion', {'id': 's-filing-loyalty'}),
          CoreCall('rejectSuggestion', {'id': 's-rel-contradicts'}),
          CoreCall('rejectSuggestion', {'id': 's-rel-part-of'}),
        ],
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
      await tester.pump();
      expect(opened, ['n-capture-loyalty']);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pump();
      expect(
        fake.calls.last,
        const CoreCall('acceptSuggestion', {'id': 's-who-is-baba'}),
      );
    });

    testWidgets('error state and refused intent', (tester) async {
      final v = matrix().first;
      final fake = FakeCoreApi();
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        settle: false,
      );
      fake.inbox.addError(StrataFixtures.coreFailure);
      await tester.pump();
      final s = lookupInboxLocalizations(v.locale);
      expect(find.text(s.inboxLoadError), findsOneWidget);
    });
  });
}
