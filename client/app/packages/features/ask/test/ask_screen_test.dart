import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/src/generated/ask_localizations.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';

/// Scrolls the conversation until [finder] is built and visible.
Future<void> reveal(WidgetTester tester, Finder finder) async {
  await tester.scrollUntilVisible(
    finder,
    150,
    scrollable: find.byType(Scrollable).first,
  );
  expect(finder, findsOneWidget);
}

FakeCoreApi _fake(AskView view) => FakeCoreApi()
  ..askStream.add(view)
  ..resolveCitationAnswer.returns(AskFixtures.preview);

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  Widget screen,
  FakeCoreApi fake,
) => pumpVariant(tester, v, screen, fake: fake, scaffold: true);

final Variant _compact = variants(scales: const [1]).first;
final Variant _expanded = variants(
  sizes: const {'expanded': StrataTestSizes.expanded},
  scales: const [1],
).first;

void main() {
  group('AskScreen matrix', () {
    for (final v in variants()) {
      testWidgets('conversation $v', (tester) async {
        final opened = <(String, String?)>[];
        final fake = await _pump(
          tester,
          v,
          AskScreen(onOpenNote: (id, anchor) => opened.add((id, anchor))),
          _fake(AskFixtures.available),
        );
        final l10n = lookupAskLocalizations(v.locale);
        expectNoErrors(tester);
        expect(fake.calls, contains(const CoreCall('watchAsk')));
        expect(
          directionOf(tester, find.byType(AskScreen)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        // The AI status and the scopes from the core.
        expect(find.text('62% used'), findsOneWidget);
        expect(find.widgetWithText(ChoiceChip, 'All notes'), findsOneWidget);
        expect(
          find.widgetWithText(ChoiceChip, 'Acme Logistics'),
          findsOneWidget,
        );
        await reveal(
          tester,
          find.text('What did Acme ask for on invoicing, and did we agree?'),
        );
        // Inline citation markers from the core's spans.
        expect(
          find.bySemanticsLabel(l10n.citationMarker(index: 1)),
          findsOneWidget,
        );
        expect(
          find.bySemanticsLabel(l10n.citationMarker(index: 2)),
          findsOneWidget,
        );
        expect(find.text('Scope: All notes'), findsWidgets);
        await reveal(tester, find.text(l10n.sources(count: 2)));
        expect(find.text('Weekly invoicing proposal'), findsOneWidget);
        await reveal(tester, find.text(l10n.openSavedNote));
        expect(find.bySemanticsLabel(l10n.conversation), findsOneWidget);
        final expanded = v.sizeClass == SizeClass.expanded;
        expect(
          find.text(l10n.sourcePreview),
          expanded ? findsOneWidget : findsNothing,
        );
        await expectAccessible(tester, contrast: v.textScale == 1);
        await tapVisible(
          tester,
          find.bySemanticsLabel(l10n.citationMarker(index: 1)),
        );
        if (expanded) {
          expect(opened, isEmpty);
          expect(
            fake.calls,
            contains(
              const CoreCall('resolveCitation', {
                'noteId': 'n-call-2026-09-12-acme',
                'anchor': 'a1b2',
              }),
            ),
          );
          expect(
            find.text('Ahmed asked for weekly invoicing from October.'),
            findsOneWidget,
          );
          expect(find.text('12 Sep 2026'), findsOneWidget);
          await tapVisible(tester, find.text(l10n.openAtBlock));
        }
        expect(opened, [('n-call-2026-09-12-acme', 'a1b2')]);
      });
    }
  });

  group('AskScreen intents', () {
    testWidgets('asks in the chosen scope', (tester) async {
      final fake = await _pump(
        tester,
        _compact,
        const AskScreen(),
        _fake(AskFixtures.empty),
      );
      final send = find.widgetWithIcon(IconButton, Icons.arrow_upward);
      expect(tester.widget<IconButton>(send).onPressed, isNull);
      await tapVisible(
        tester,
        find.widgetWithText(ChoiceChip, 'Acme Logistics'),
      );
      await tester.enterText(find.byType(TextField), 'What is open with Acme?');
      await settle(tester);
      await tester.tap(send);
      await settle(tester);
      expect(fake.calls.last.method, 'ask');
      expect(fake.calls.last.args['question'], 'What is open with Acme?');
      final scope = fake.calls.last.args['scope']! as AskScope;
      expect(
        (scope.kind, scope.value, scope.label),
        (AskScopeKind.entity, 'c-acme-logistics', 'Acme Logistics'),
      );
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty,
      );
    });

    testWidgets('a streaming answer can be stopped', (tester) async {
      final fake = await _pump(
        tester,
        _compact,
        const AskScreen(),
        _fake(AskFixtures.streaming),
      );
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      expect(find.bySemanticsLabel('Answering…'), findsOneWidget);
      expect(find.text('Save as note'), findsNothing);
      expect(
        tester
            .widget<IconButton>(
              find.widgetWithIcon(IconButton, Icons.add_comment_outlined),
            )
            .onPressed,
        isNull,
      );
      await tester.tap(find.byTooltip('Stop'));
      await settle(tester);
      expect(fake.calls.last, const CoreCall('stopAsk'));
    });

    testWidgets('an answer cut short says why', (tester) async {
      await _pump(
        tester,
        _compact,
        const AskScreen(),
        _fake(AskFixtures.paused),
      );
      expect(
        find.text('AI is paused — this answer stopped early.'),
        findsOneWidget,
      );
      expect(find.text('Paused until 14:00'), findsOneWidget);
    });

    testWidgets('about a note: its thread, its questions, back to all notes', (
      tester,
    ) async {
      String? opened;
      final fake = await _pump(
        tester,
        _compact,
        AskScreen(onOpenNote: (id, _) => opened = id),
        _fake(AskFixtures.aboutNote),
      );
      // The note replaces the scope chips; the hint says what is asked about.
      expect(find.text('About Call 2026-09-12 Acme'), findsOneWidget);
      expect(find.byType(ChoiceChip), findsNothing);
      expect(find.text('Ask about this note'), findsOneWidget);

      await tester.enterText(find.byType(TextField), 'And the payment terms?');
      await settle(tester);
      await tester.tap(find.widgetWithIcon(IconButton, Icons.arrow_upward));
      await settle(tester);
      final scope = fake.calls.last.args['scope']! as AskScope;
      expect(
        (fake.calls.last.method, scope.kind, scope.value),
        ('ask', AskScopeKind.note, 'n-call-2026-09-12-acme'),
      );

      await tester.tap(find.text('About Call 2026-09-12 Acme'));
      await settle(tester);
      expect(opened, 'n-call-2026-09-12-acme');
      await tester.tap(find.byTooltip('Ask about all notes'));
      await settle(tester);
      expect(fake.calls.last, const CoreCall('newConversation'));
    });

    testWidgets('new conversation', (tester) async {
      final fake = await _pump(
        tester,
        _compact,
        const AskScreen(),
        _fake(AskFixtures.available),
      );
      await tester.tap(find.byTooltip('New conversation'));
      await settle(tester);
      expect(fake.calls.last, const CoreCall('newConversation'));
    });

    testWidgets('save as note, then open it', (tester) async {
      final opened = <(String, String?)>[];
      final fake = await _pump(
        tester,
        _compact,
        AskScreen(onOpenNote: (id, anchor) => opened.add((id, anchor))),
        _fake(AskFixtures.available),
      );
      await tapVisible(tester, find.text('Save as note'));
      expect(
        fake.calls.last,
        const CoreCall('saveAnswerAsNote', {'messageId': 'a-1'}),
      );
      expect(find.text('Saved as a note'), findsOneWidget);
      await tapVisible(tester, find.text('Saved · Open note'));
      expect(opened, [('n-ask-nile-freight', null)]);
    });

    testWidgets('a failed intent is reported', (tester) async {
      final fake = _fake(AskFixtures.available)
        ..newConversationAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await _pump(tester, _compact, const AskScreen(), fake);
      await tester.tap(find.byTooltip('New conversation'));
      await settle(tester);
      expect(
        find.text("The app's local data returned an error (offline)."),
        findsOneWidget,
      );
    });

    testWidgets('a source row opens its first cited block', (tester) async {
      final opened = <(String, String?)>[];
      await _pump(
        tester,
        _compact,
        AskScreen(onOpenNote: (id, anchor) => opened.add((id, anchor))),
        _fake(AskFixtures.available),
      );
      await tapVisible(tester, find.text('Weekly invoicing proposal'));
      expect(opened, [('n-weekly-invoicing', 'd2e5')]);
    });

    testWidgets('copy answer puts the answer on the clipboard', (tester) async {
      final copied = <Object?>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') copied.add(call.arguments);
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );
      await _pump(
        tester,
        _expanded,
        const AskScreen(),
        _fake(AskFixtures.available),
      );
      await tapVisible(tester, find.byTooltip('Copy answer').first);
      expect(copied, [
        {'text': AskFixtures.answer.text},
      ]);
      expect(find.text('Answer copied'), findsOneWidget);
    });
  });

  group('AskScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await _pump(tester, v, const AskScreen(), FakeCoreApi());
        expect(
          find.bySemanticsLabel(lookupAskLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('offline $v', (tester) async {
        await _pump(tester, v, const AskScreen(), _fake(AskFixtures.offline));
        final l10n = lookupAskLocalizations(v.locale);
        expect(find.text(l10n.offlineTitle), findsOneWidget);
        expect(find.text(l10n.emptyTitle), findsOneWidget);
        expect(tester.widget<TextField>(find.byType(TextField)).enabled, false);
        await expectAccessible(tester);
      });

      testWidgets('not available yet $v', (tester) async {
        await _pump(tester, v, const AskScreen(), _fake(AskFixtures.notYet));
        final l10n = lookupAskLocalizations(v.locale);
        expect(find.text(l10n.notYetTitle), findsOneWidget);
        expect(find.text(l10n.scope), findsNothing);
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi();
        await _pump(tester, v, const AskScreen(), fake);
        fake.askStream.addError(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
        await settle(tester);
        expect(
          find.text(
            lookupAskLocalizations(v.locale).errorMessage(code: 'offline'),
          ),
          findsOneWidget,
        );
      });
    }
  });
}
