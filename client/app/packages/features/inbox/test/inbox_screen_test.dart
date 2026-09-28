import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';

const _acme =
    'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري ابتداءً من '
    'أكتوبر';

Future<void> _tap(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isEmpty) {
    await tester.scrollUntilVisible(finder, 200, scrollable: _list);
  }
  await tapVisible(tester, finder.first);
}

/// The inbox list's scrollable (not a text field's).
final Finder _list = find
    .descendant(of: find.byType(ListView), matching: find.byType(Scrollable))
    .first;

/// Scrolls [finder] into view when the list has not built it yet.
Future<void> _reveal(WidgetTester tester, Finder finder) async {
  if (finder.evaluate().isNotEmpty) return;
  await tester.scrollUntilVisible(finder, 200, scrollable: _list);
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
    for (final v in variants()) {
      testWidgets('structure, intents and accessibility [$v]', (tester) async {
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.full);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          scaffold: true,
        );
        final s = lookupInboxLocalizations(v.locale);
        expectNoErrors(tester);
        expect(
          fake.calls,
          contains(
            const CoreCall('watchInboxFiltered', {'filter': InboxFilter.all}),
          ),
        );
        // The core's filters with counts, and "Accept all ready".
        expect(find.text(s.inboxFilterAll(count: 4)), findsOneWidget);
        expect(find.text(s.inboxFilterNeedsYou(count: 1)), findsOneWidget);
        expect(find.text(s.inboxAcceptAllReady(count: 1)), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);

        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(find.byType(StrataPanes), findsNothing);
            expect(find.text(s.inboxWhoIs(mention: 'بابا')), findsOneWidget);
            await _reveal(tester, find.text('Weekly invoicing request — Acme'));
            await _tap(tester, _inCard<CaptureCard>(_acme, s.inboxAccept));
            expect(
              fake.calls.last,
              const CoreCall('acceptCapture', {'noteId': 'n-capture-acme'}),
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
              fake.calls.last,
              const CoreCall('rejectCapture', {'noteId': 'n-capture-acme'}),
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
            final bulkAccept = _only(fake, 'acceptCaptures');
            expect(bulkAccept.args['noteIds'], [
              'n-capture-acme',
              'n-capture-loyalty',
              'n-capture-nile',
            ]);
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
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.full);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(initialNoteId: 'n-capture-loyalty'),
          fake: fake,
          scaffold: true,
        );
        final strata = v.l10n;
        final chip = find.bySemanticsLabel(
          RegExp('^${strata.relationContradicts}: Discount policy'),
        );
        if (chip.evaluate().isEmpty) {
          await tester.scrollUntilVisible(chip, 200, scrollable: _list);
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
    for (final v in variants()) {
      testWidgets('link-or-create: create person, alias, reject [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.linkOrCreate);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          scaffold: true,
        );
        final s = lookupInboxLocalizations(v.locale);
        expect(find.text(s.inboxWhoIs(mention: 'بابا')), findsWidgets);
        expect(find.text(s.inboxNoPersonMatches), findsWidgets);
        expect(find.text(s.inboxAliasNote(mention: 'بابا')), findsWidgets);
        await expectAccessible(tester, contrast: v.textScale == 1);
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
        expect(
          fake.calls.last,
          const CoreCall('resolveLinkOrCreate', {
            'id': 's-who-is-baba',
            'choice': LinkOrCreateChoice(
              kind: LinkOrCreateKind.create,
              name: 'Ibrahim Shawket',
              force: false,
            ),
          }),
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
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.custody);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          scaffold: true,
        );
        final s = lookupInboxLocalizations(v.locale);
        expect(find.text(s.inboxAppliedAutomatically), findsWidgets);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
        if (v.sizeClass == SizeClass.compact) {
          expect(find.text(s.inboxAiConfidence(score: '0.93')), findsOneWidget);
          await _reveal(
            tester,
            find.text(s.inboxAfterAt(place: 'Safe — Nasr City office')),
          );
          expect(
            find.text(s.inboxAfterLastWith(person: 'Shady')),
            findsOneWidget,
          );
          await _reveal(tester, find.text(s.inboxWhichDocument));
          expect(find.text(s.inboxWhichDocument), findsOneWidget);
          await _reveal(tester, find.text('Which paper did you give Shady?'));
          expect(find.text('Which paper did you give Shady?'), findsOneWidget);
          // The ambiguous event's documents are the answers.
          await _tap(tester, find.text('Petrol Arrows commercial register'));
          expect(
            fake.calls.last,
            const CoreCall('acceptSuggestionChoice', {
              'id': 's-custody-which',
              'documentId': 'd-petrol-arrows-register',
            }),
          );
          tester.state<ScrollableState>(_list).position.jumpTo(0);
          await settle(tester);
          await _tap(tester, find.text(s.inboxLooksRight));
          expect(
            fake.calls.last,
            const CoreCall('acknowledgeSuggestion', {
              'id': 's-custody-applied',
            }),
          );
        }
        await _tap(tester, find.text(s.inboxUndo));
        expect(
          fake.calls.last,
          const CoreCall('undoSuggestion', {'id': 's-custody-applied'}),
        );
      });

      testWidgets('duplicate-flagged, task and unsupported [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.others);
        final opened = <String>[];
        await pumpVariant(
          tester,
          v,
          InboxScreen(onOpenNote: opened.add),
          fake: fake,
          scaffold: true,
        );
        final s = lookupInboxLocalizations(v.locale);
        final t = lookupTasksLocalizations(v.locale);
        expect(find.text(s.inboxPossibleDuplicate), findsWidgets);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
        await _tap(tester, find.text(t.dupCreateAnyway));
        expect(
          fake.calls.last,
          const CoreCall('resolveCaptureDuplicate', {
            'id': 's-duplicate-watanya',
            'choice': DuplicateChoice.createAnyway,
          }),
        );
        await _tap(tester, find.text(s.inboxDiscard));
        expect(
          fake.calls.last,
          const CoreCall('resolveCaptureDuplicate', {
            'id': 's-duplicate-watanya',
            'choice': DuplicateChoice.discard,
          }),
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

      testWidgets('a duplicates pair says what Merge keeps [$v]', (
        tester,
      ) async {
        final fake = FakeCoreApi()
          ..inboxFiltered[InboxFilter.all].add(InboxFixtures.duplicates);
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          scaffold: true,
        );
        final s = lookupInboxLocalizations(v.locale);
        await _reveal(
          tester,
          find.text(InboxFixtures.duplicatesPair.detail.mergeLabel!),
        );
        expect(
          find.text(InboxFixtures.duplicatesPair.detail.mergeLabel!),
          findsOneWidget,
        );
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
        await _tap(tester, find.text(s.inboxMerge));
        expect(
          fake.calls.last,
          const CoreCall('acceptSuggestion', {'id': 's-duplicates-eta'}),
        );
        await _tap(tester, find.text(s.inboxKeepBoth));
        expect(
          fake.calls.last,
          const CoreCall('rejectSuggestion', {'id': 's-duplicates-eta'}),
        );
      });

      testWidgets('empty, loading and error [$v]', (tester) async {
        final fake = FakeCoreApi();
        await pumpVariant(
          tester,
          v,
          const InboxScreen(),
          fake: fake,
          scaffold: true,
        );
        final t = lookupTasksLocalizations(v.locale);
        final s = lookupInboxLocalizations(v.locale);
        expect(find.bySemanticsLabel(t.commonLoading), findsOneWidget);
        fake.inboxFiltered[InboxFilter.all].add(InboxFixtures.empty);
        await tester.pump();
        await tester.pump();
        expect(find.text(s.inboxEmptyTitle), findsOneWidget);
        await expectAccessible(tester, contrast: v.textScale == 1);
        expectNoErrors(tester);
      });
    }
  });

  group('InboxScreen keyboard and errors', () {
    testWidgets('expanded: J / X / A / R / E', (tester) async {
      final v = variants().firstWhere((v) => v.sizeName == 'expanded');
      final fake = FakeCoreApi()
        ..inboxFiltered[InboxFilter.all].add(InboxFixtures.full);
      final opened = <String>[];
      await pumpVariant(
        tester,
        v,
        InboxScreen(onOpenNote: opened.add),
        fake: fake,
        scaffold: true,
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
        fake.calls.last,
        const CoreCall('rejectCapture', {'noteId': 'n-capture-loyalty'}),
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

    testWidgets('filters, accept all ready', (tester) async {
      final v = variants().first;
      final fake = FakeCoreApi()
        ..inboxFiltered[InboxFilter.all].add(InboxFixtures.full)
        ..inboxFiltered[InboxFilter.needsYou].add(InboxFixtures.linkOrCreate);
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        scaffold: true,
      );
      final s = lookupInboxLocalizations(v.locale);
      await _tap(tester, find.text(s.inboxAcceptAllReady(count: 1)));
      expect(fake.calls.last, const CoreCall('acceptAllReady'));
      await _tap(tester, find.text(s.inboxFilterNeedsYou(count: 1)));
      expect(
        fake.calls.last,
        const CoreCall('watchInboxFiltered', {'filter': InboxFilter.needsYou}),
      );
    });

    testWidgets('edit a filing before accepting it', (tester) async {
      final v = variants().first;
      final fake = FakeCoreApi()
        ..inboxFiltered[InboxFilter.all].add(InboxFixtures.full);
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        scaffold: true,
      );
      final s = lookupInboxLocalizations(v.locale);
      await _reveal(tester, find.text('Weekly invoicing request — Acme'));
      await _tap(tester, _inCard<CaptureCard>(_acme, s.inboxEdit));
      expect(find.byType(EditProposalSheet), findsOneWidget);
      await tester.enterText(
        find.widgetWithText(TextField, s.inboxEditNoteTitle),
        'Acme weekly invoicing',
      );
      await _tap(tester, find.text(s.inboxAcceptEdited));
      expect(
        fake.calls.last,
        const CoreCall('acceptSuggestionWith', {
          'id': 's-filing-acme',
          'edits': SuggestionEdits(
            title: 'Acme weekly invoicing',
            folder: 'notes/clients/acme',
          ),
        }),
      );
    });

    testWidgets('reply to the AI in a suggestion thread', (tester) async {
      final v = variants().first;
      final fake = FakeCoreApi()
        ..inboxFiltered[InboxFilter.all].add(InboxFixtures.custody);
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        scaffold: true,
      );
      final s = lookupInboxLocalizations(v.locale);
      await _reveal(tester, find.widgetWithText(TextField, s.inboxReplyField));
      await tester.enterText(
        find.widgetWithText(TextField, s.inboxReplyField),
        'The Watanya contract',
      );
      await settle(tester);
      await _tap(tester, find.byTooltip(s.inboxReplySend));
      expect(
        fake.calls.last,
        const CoreCall('replyToSuggestion', {
          'id': 's-custody-which',
          'text': 'The Watanya contract',
        }),
      );
    });

    testWidgets('link the mention to a candidate', (tester) async {
      final v = variants().first;
      final baba = StrataFixtures.suggestionLinkOrCreate;
      final fake = FakeCoreApi()
        ..inboxFiltered[InboxFilter.all].add(
          InboxView(
            captures: const [],
            suggestions: [
              SuggestionItem(
                id: baba.id,
                noteId: baba.noteId,
                status: baba.status,
                detail: const SuggestionDetail(
                  kind: SuggestionKind.entityLink,
                  title: '',
                  folder: '',
                  tags: [],
                  mention: 'بابا',
                  candidates: [StrataFixtures.ahmedSamirRef],
                  target: EntityRef(
                    id: 'p-shawket-sr',
                    title: 'Ibrahim Shawket',
                  ),
                  line: '',
                  confidence: 0.7,
                  duplicates: [],
                  relType: '',
                  reason: '“بابا” is how you refer to your father.',
                  serverKind: 'entity_link',
                  documentChoices: [],
                  entityKind: 'person',
                  isNickname: true,
                  quote: '',
                  entities: [],
                ),
                created: baba.created,
                pendingSync: false,
                createdLabel: baba.createdLabel,
                sourceDir: TextDir.rtl,
                autoApplied: false,
                canAccept: false,
                needsYou: true,
                thread: const [],
              ),
            ],
            filter: InboxFilter.all,
            readyCount: 0,
            needsYouCount: 1,
            conflictsCount: 0,
            allCount: 1,
          ),
        );
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        scaffold: true,
      );
      final s = lookupInboxLocalizations(v.locale);
      expect(find.text(s.inboxNickname), findsOneWidget);
      expect(
        find.text('“بابا” is how you refer to your father.'),
        findsOneWidget,
      );
      // The AI's proposal first, then the other candidates.
      final links = tester
          .widgetList<Text>(
            find.descendant(
              of: find.byType(OutlinedButton),
              matching: find.byType(Text),
            ),
          )
          .map((t) => t.data)
          .toList();
      expect(links, [
        s.inboxItIs(title: 'Ibrahim Shawket'),
        s.inboxItIs(title: 'Ahmed Samir'),
      ]);
      await _tap(tester, find.text(s.inboxItIs(title: 'Ibrahim Shawket')));
      expect(
        fake.calls.last,
        const CoreCall('resolveLinkOrCreate', {
          'id': 's-who-is-baba',
          'choice': LinkOrCreateChoice(
            kind: LinkOrCreateKind.link,
            entityId: 'p-shawket-sr',
            force: false,
          ),
        }),
      );
    });

    testWidgets('error state and refused intent', (tester) async {
      final v = variants().first;
      final fake = FakeCoreApi();
      await pumpVariant(
        tester,
        v,
        const InboxScreen(),
        fake: fake,
        scaffold: true,
      );
      fake.inboxFiltered[InboxFilter.all].addError(StrataFixtures.coreFailure);
      await tester.pump();
      final s = lookupInboxLocalizations(v.locale);
      expect(find.text(s.inboxLoadError), findsOneWidget);
    });
  });
}
