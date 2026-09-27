import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

const _op = 'op-weekly';

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v, {
  ConflictDetail? detail = SyncFixtures.detail,
  Object? error,
  bool emit = true,
  VoidCallback? onResolved,
  VoidCallback? onClose,
}) async {
  final fake = FakeCoreApi();
  if (emit && error == null) {
    fake.conflict[_op].add(ConflictScreen(opId: _op, conflict: detail));
  }
  await pumpVariant(
    tester,
    v,
    Scaffold(
      body: ConflictResolutionScreen(
        opId: _op,
        onResolved: onResolved,
        onClose: onClose,
      ),
    ),
    fake: fake,
  );
  if (error != null) {
    fake.conflict[_op].addError(error);
    await settle(tester);
  }
  return fake;
}

/// Asserts the last call resolved [_op] with exactly these fields (frb
/// compares lists by identity, so the choices are compared one by one).
void _expectResolution(
  FakeCoreApi fake,
  ResolutionKind kind, {
  String? content,
  List<HunkChoice> choices = const [],
}) {
  final call = fake.calls.last;
  expect(call.method, 'resolveConflict');
  expect(call.args['opId'], _op);
  final resolution = call.args['resolution']! as ConflictResolution;
  expect(resolution.kind, kind);
  expect(resolution.content, content);
  expect(resolution.choices, orderedEquals(choices));
}

Future<void> _tapVisible(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await settle(tester);
}

void main() {
  group('conflict matrix', () {
    for (final v in matrix()) {
      testWidgets('hunks to choose $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        final fake = await _pump(tester, v);
        expect(
          fake.calls.first,
          const CoreCall('watchConflict', {'opId': _op}),
        );
        expect(find.text('Weekly invoicing proposal'), findsOneWidget);
        expect(find.text(l10n.conflictIntro), findsOneWidget);
        if (v.sizeClass == SizeClass.expanded) {
          // Three columns side by side.
          expect(find.byType(TabBar), findsNothing);
          expect(find.text(l10n.columnDevice), findsOneWidget);
          expect(find.text(l10n.columnServer), findsOneWidget);
          expect(find.textContaining(l10n.columnMerged), findsOneWidget);
          expect(
            find.textContaining('net 14', findRichText: true),
            findsWidgets,
          );
        } else {
          // Tabs, the merged result first.
          expect(find.byType(TabBar), findsOneWidget);
          expect(find.byType(Tab), findsNWidgets(3));
        }
        expect(find.text(l10n.hunkTitle(location: 'body:6')), findsOneWidget);
        expect(find.text(l10n.hunksLeft(count: 2)), findsOneWidget);
        final merged = tester.widget<FilledButton>(
          find.widgetWithText(FilledButton, l10n.keepMerged),
        );
        expect(merged.onPressed, isNull);
        expect(
          find.byType(KeyboardHintChip),
          v.sizeClass == SizeClass.compact ? findsNothing : findsOneWidget,
        );
        expect(
          directionOf(tester, find.byType(ConflictResolutionView)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('resolved or unknown $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _pump(tester, v, detail: null);
        expect(find.text(l10n.conflictGone), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester);
      });

      testWidgets('error $v', (tester) async {
        final l10n = lookupSyncLocalizations(v.locale);
        await _pump(
          tester,
          v,
          error: const CoreFailure(
            code: 'not_found',
            messageKey: 'error.not_found',
          ),
        );
        expect(find.text(l10n.loadFailed), findsOneWidget);
        expect(find.text(l10n.errorNotFound), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('conflict intents', () {
    final compact = matrix().first;
    final expanded = matrix().firstWhere((v) => v.sizeName == 'expanded');

    testWidgets('loading until the core emits', (tester) async {
      await _pump(tester, compact, emit: false);
      expect(find.bySemanticsLabel('Loading sync status'), findsOneWidget);
    });

    testWidgets('Keep server and Keep this device', (tester) async {
      var resolved = 0;
      final fake = await _pump(tester, expanded, onResolved: () => resolved++);
      await _tapVisible(tester, find.text('Keep server'));
      _expectResolution(fake, ResolutionKind.keepServer);
      await _tapVisible(tester, find.text('Keep this device'));
      _expectResolution(fake, ResolutionKind.keepMine);
      expect(resolved, 2);
    });

    testWidgets('Keep merged sends one choice per hunk', (tester) async {
      final fake = await _pump(tester, expanded);
      await _tapVisible(tester, find.text("Keep the server's text").first);
      expect(find.text('1 choice left'), findsOneWidget);
      // The frontmatter hunk offers no line-level choices.
      expect(find.text('Keep both'), findsOneWidget);
      await _tapVisible(tester, find.text("Keep this device's text").last);
      expect(find.text('Every choice made'), findsOneWidget);
      await _tapVisible(tester, find.text('Keep merged'));
      _expectResolution(
        fake,
        ResolutionKind.hunks,
        choices: const [
          HunkChoice(hunk: 0, choice: HunkChoiceKind.theirs),
          HunkChoice(hunk: 1, choice: HunkChoiceKind.ours),
        ],
      );
    });

    testWidgets('Write my own sends the typed text', (tester) async {
      final fake = await _pump(tester, expanded);
      await _tapVisible(tester, find.text('Write my own'));
      await tester.enterText(
        find.widgetWithText(TextField, 'Your text for body:6'),
        '- Payment terms: net 10.\n',
      );
      await _tapVisible(tester, find.text('Keep the original').last);
      await _tapVisible(tester, find.text('Keep merged'));
      _expectResolution(
        fake,
        ResolutionKind.hunks,
        choices: const [
          HunkChoice(
            hunk: 0,
            choice: HunkChoiceKind.text,
            text: '- Payment terms: net 10.\n',
          ),
          HunkChoice(hunk: 1, choice: HunkChoiceKind.base),
        ],
      );
    });

    testWidgets('an edited merged text is sent as merged content', (
      tester,
    ) async {
      final fake = await _pump(tester, expanded);
      await tester.enterText(
        find.widgetWithText(TextField, 'Merged note text'),
        'Merged by hand.\n',
      );
      await tester.pump();
      await _tapVisible(tester, find.text('Keep merged'));
      _expectResolution(
        fake,
        ResolutionKind.merged,
        content: 'Merged by hand.\n',
      );
    });

    testWidgets('a clean merge keeps the preview (and Cmd-Enter)', (
      tester,
    ) async {
      final fake = await _pump(tester, expanded, detail: SyncFixtures.clean);
      expect(
        find.text(
          'This device and the server both edited this note. The changes '
          "don't overlap and merged cleanly.",
        ),
        findsOneWidget,
      );
      await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
      await settle(tester);
      _expectResolution(
        fake,
        ResolutionKind.merged,
        content: SyncFixtures.clean.mergedPreview,
      );
    });

    testWidgets('a refused resolution shows the core error', (tester) async {
      final fake = FakeCoreApi()
        ..resolveConflictAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      fake.conflict[_op].add(
        const ConflictScreen(opId: _op, conflict: SyncFixtures.detail),
      );
      await pumpVariant(
        tester,
        expanded,
        const Scaffold(body: ConflictResolutionScreen(opId: _op)),
        fake: fake,
      );
      await _tapVisible(tester, find.text('Keep server'));
      expect(
        find.text("The conflict could not be resolved: You're offline."),
        findsOneWidget,
      );
    });

    testWidgets('Close and decide later', (tester) async {
      var closed = 0;
      await _pump(tester, compact, onClose: () => closed++);
      await tester.tap(find.byTooltip('Close and decide later'));
      expect(closed, 1);
    });

    testWidgets('compact tabs show each version', (tester) async {
      await _pump(tester, compact);
      await tester.tap(find.text('Server'));
      await settle(tester);
      expect(find.textContaining('net 7', findRichText: true), findsWidgets);
      await tester.tap(find.text('This device'));
      await settle(tester);
      expect(find.textContaining('net 14', findRichText: true), findsWidgets);
    });
  });
}
