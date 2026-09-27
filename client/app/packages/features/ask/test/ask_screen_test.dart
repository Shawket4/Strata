import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/src/generated/ask_localizations.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

/// Scrolls the conversation until [finder] is built and visible.
Future<void> reveal(WidgetTester tester, Finder finder) async {
  await tester.scrollUntilVisible(
    finder,
    150,
    scrollable: find.byType(Scrollable).first,
  );
  expect(finder, findsOneWidget);
}

void main() {
  group('AskScreen matrix', () {
    for (final v in variants()) {
      testWidgets('conversation $v', (tester) async {
        final fake = FakeCoreApi();
        fake.note['n-call-2026-09-12-acme'].add(StrataFixtures.noteScreen);
        final opened = <(String, String?)>[];
        await pumpVariant(
          tester,
          v,
          AskScreen(onOpenNote: (id, anchor) => opened.add((id, anchor))),
          fake,
        );
        final l10n = lookupAskLocalizations(v.locale);
        expectNoErrors(tester);
        expect(fake.calls, contains(const CoreCall('askView')));
        expect(find.text(l10n.notYetTitle), findsOneWidget);
        await reveal(tester, find.text('What did Ahmed ask for?'));
        await reveal(
          tester,
          find.text('Ahmed asked for weekly invoicing starting in October.'),
        );
        await reveal(tester, find.text(l10n.sources(count: 1)));
        expect(find.bySemanticsLabel(l10n.conversation), findsOneWidget);
        final send = tester.widget<IconButton>(
          find.widgetWithIcon(IconButton, Icons.arrow_upward),
        );
        expect(send.onPressed, isNull);
        final expanded = v.sizeClass == SizeClass.expanded;
        expect(
          find.text(l10n.sourcePreview),
          expanded ? findsOneWidget : findsNothing,
        );
        if (v.textScale == 1) await expectAccessible(tester);
        final chip = find.bySemanticsLabel(
          lookupStrataLocalizations(v.locale)
              .citationSemantics(label: 'Call 2026-09-12 — Acme'),
        );
        await tester.ensureVisible(chip);
        await tester.pump();
        await tester.tap(chip);
        await tester.pump();
        await tester.pump();
        if (expanded) {
          expect(opened, isEmpty);
          expect(
            fake.calls,
            contains(
              const CoreCall('watchNote', {'id': 'n-call-2026-09-12-acme'}),
            ),
          );
          await tester.tap(find.text(l10n.openAtBlock));
        }
        expect(opened, [('n-call-2026-09-12-acme', 'a1b2')]);
      });
    }
  });

  group('AskScreen states', () {
    for (final v in variants(scales: const [1])) {
      testWidgets('loading $v', (tester) async {
        await pumpVariant(
          tester,
          v,
          const AskScreen(),
          FakeCoreApi(),
          overrides: [
            askViewProvider.overrideWith((ref) => Completer<AskView>().future),
          ],
        );
        expect(
          find.bySemanticsLabel(lookupAskLocalizations(v.locale).loading),
          findsOneWidget,
        );
      });

      testWidgets('offline $v', (tester) async {
        final fake = FakeCoreApi()..askViewAnswer.returns(AskFixtures.offline);
        await pumpVariant(tester, v, const AskScreen(), fake);
        final l10n = lookupAskLocalizations(v.locale);
        expect(find.text(l10n.offlineTitle), findsOneWidget);
        expect(find.text(l10n.emptyTitle), findsOneWidget);
        await expectAccessible(tester);
      });

      testWidgets('available, mixed scripts $v', (tester) async {
        final fake = FakeCoreApi()
          ..askViewAnswer.returns(AskFixtures.available);
        await pumpVariant(tester, v, const AskScreen(), fake);
        final l10n = lookupAskLocalizations(v.locale);
        expectNoErrors(tester);
        expect(find.text(l10n.notYetTitle), findsNothing);
        expect(find.text(l10n.offlineTitle), findsNothing);
        await reveal(tester, find.text(l10n.sources(count: 2)));
        await reveal(tester, find.text('ومنى قالت إيه عن أسعار Nile Freight؟'));
        await expectAccessible(tester);
      });

      testWidgets('empty $v', (tester) async {
        final fake = FakeCoreApi()..askViewAnswer.returns(AskFixtures.empty);
        await pumpVariant(tester, v, const AskScreen(), fake);
        expect(
          find.text(lookupAskLocalizations(v.locale).emptyTitle),
          findsOneWidget,
        );
      });

      testWidgets('error $v', (tester) async {
        final fake = FakeCoreApi()
          ..askViewAnswer.throws(
            const CoreFailure(code: 'offline', messageKey: 'error.offline'),
          );
        await pumpVariant(tester, v, const AskScreen(), fake);
        expect(
          find.text(
            lookupAskLocalizations(v.locale).errorMessage(code: 'offline'),
          ),
          findsOneWidget,
        );
      });
    }
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
    await pumpVariant(
      tester,
      variants().first,
      const AskScreen(),
      FakeCoreApi(),
    );
    await tester.tap(find.byTooltip('Copy answer'));
    await tester.pump();
    expect(copied, [
      {'text': 'Ahmed asked for weekly invoicing starting in October.'},
    ]);
    expect(find.text('Answer copied'), findsOneWidget);
  });
}
