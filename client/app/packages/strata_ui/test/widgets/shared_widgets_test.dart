import 'dart:ui' show Tristate;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

import '../helpers/gallery.dart';
import '../helpers/harness.dart';

abstract class _Callback {
  void call();
}

class _MockCallback extends Mock implements _Callback;

Widget _galleryPage() => const Scaffold(
  body: SingleChildScrollView(
    padding: EdgeInsets.all(StrataSpacing.s4),
    child: WidgetGallery(),
  ),
);

Future<void> _pumpSingle(WidgetTester tester, Widget child) async {
  final v = variants(textScales: const [1]).first;
  await pumpVariant(tester, v, Scaffold(body: Center(child: child)));
}

void main() {
  group('shared widgets matrix', () {
    for (final v in variants()) {
      testWidgets('$v', (tester) async {
        await pumpVariant(tester, v, _galleryPage());
        final l10n = await StrataLocalizations.delegate.load(v.locale);
        final colors = v.theme.extension<StrataColors>()!;

        // Relation chips: nine types, AI tag only on AI-made relations.
        expect(find.byType(RelationChip), findsNWidgets(9));
        expect(find.byType(RelationLineSample), findsNWidgets(9));
        expect(find.text('${l10n.aiTag} · 0.82'), findsOneWidget);
        expect(find.textContaining('${l10n.aiTag} · '), findsNWidgets(3));
        expect(
          find.bySemanticsLabel(
            '${l10n.relationFollowsUp}: ${WidgetGallery.aiRelation}, '
            '${l10n.aiConfidenceSemantics(confidence: '0.82')}',
          ),
          findsOneWidget,
        );
        expect(
          find.bySemanticsLabel('${l10n.relationPartOf}: Subscription tiers'),
          findsOneWidget,
        );
        for (final painter
            in tester
                .widgetList<CustomPaint>(
                  find.descendant(
                    of: find.byType(RelationLineSample),
                    matching: find.byType(CustomPaint),
                  ),
                )
                .map((p) => p.painter! as RelationLinePainter)) {
          expect(painter.textDirection, v.direction);
        }

        // Node glyphs are announced by their localized kind.
        for (final kind in NodeKind.values) {
          expect(
            find.bySemanticsLabel(l10n.nodeKindLabel(kind)),
            findsAtLeastNWidgets(1),
          );
        }
        final selectedNote = tester
            .getSemantics(find.bySemanticsLabel(l10n.nodeKindNote).last)
            .getSemanticsData();
        expect(selectedNote.flagsCollection.isSelected, Tristate.isTrue);

        // Sync pill copy per state, plus the dense rail form.
        for (final label in [
          l10n.syncSynced(time: '14:32'),
          l10n.syncSyncing(done: 12, total: 40),
          l10n.syncConflicts(count: 1),
        ]) {
          expect(find.text(label), findsOneWidget);
        }
        expect(find.text(l10n.syncOfflineQueued(count: 3)), findsOneWidget);
        expect(
          find.bySemanticsLabel(
            '${l10n.syncOfflineQueued(count: 3)}, ${l10n.syncStatusLabel}',
          ),
          findsNWidgets(2),
        );
        final conflict = tester.widget<StatusPill>(
          find.ancestor(
            of: find.text(l10n.syncConflicts(count: 1)),
            matching: find.byType(StatusPill),
          ),
        );
        expect(conflict.tone, StatusTone.danger);
        expect(conflict.icon, Icons.report_problem_outlined);

        // Citations, keyboard hints and status pills.
        expect(
          find.bySemanticsLabel(
            l10n.citationSemantics(label: WidgetGallery.aiRelation),
          ),
          findsOneWidget,
        );
        expect(find.text('#^a1b2'), findsOneWidget);
        expect(
          find.bySemanticsLabel(l10n.shortcutSemantics(keys: '⌘ K')),
          findsOneWidget,
        );
        expect(find.byType(StatusPill), findsNWidgets(9));
        expect(find.bySemanticsLabel('AI paused'), findsOneWidget);

        // Section headers and the empty-state title are headers.
        final header = tester
            .getSemantics(find.bySemanticsLabel('Relations, 9'))
            .getSemanticsData();
        expect(header.flagsCollection.isHeader, isTrue);
        final emptyTitle = tester
            .getSemantics(find.text('Inbox zero'))
            .getSemanticsData();
        expect(emptyTitle.flagsCollection.isHeader, isTrue);

        // Interactive compact visuals get touch-sized hit areas.
        for (final element in find.byType(StrataTapTarget).evaluate()) {
          final size = tester.getSize(find.byWidget(element.widget));
          expect(size.width, greaterThanOrEqualTo(48));
          expect(size.height, greaterThanOrEqualTo(48));
        }

        // Colours come from the theme tokens.
        final bandsPainter =
            tester
                    .widget<CustomPaint>(
                      find.descendant(
                        of: find.byType(StrataBands),
                        matching: find.byType(CustomPaint),
                      ),
                    )
                    .painter!
                as StrataBandsPainter;
        expect(bandsPainter.base, colors.background);

        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('interaction wiring', () {
    testWidgets('relation chip forwards tap and long press', (tester) async {
      final tap = _MockCallback();
      final longPress = _MockCallback();
      await _pumpSingle(
        tester,
        RelationChip(
          type: RelationType.contradicts,
          label: 'Discount policy',
          aiConfidence: 0.74,
          onPressed: tap.call,
          onLongPress: longPress.call,
        ),
      );
      await tester.tap(find.text('Discount policy'));
      await tester.longPress(find.text('Discount policy'));
      verify(tap.call).called(1);
      verify(longPress.call).called(1);
    });

    testWidgets('sync pill, citation chip and empty-state action fire', (
      tester,
    ) async {
      final sync = _MockCallback();
      final cite = _MockCallback();
      final action = _MockCallback();
      await _pumpSingle(
        tester,
        SizedBox(
          width: 360,
          height: 600,
          child: Column(
            children: [
              SyncPill(
                status: const SyncConflict(count: 1),
                onPressed: sync.call,
              ),
              CitationChip(label: 'Churn notes', onPressed: cite.call),
              Expanded(
                child: StrataEmptyState(
                  title: 'Nothing here',
                  action: StrataAction(
                    label: 'Retry',
                    icon: Icons.refresh,
                    onPressed: action.call,
                  ),
                ),
              ),
            ],
          ),
        ),
      );
      await tester.tap(find.text('1 conflict'));
      await tester.tap(find.text('Churn notes'));
      await tester.tap(find.text('Retry'));
      verify(sync.call).called(1);
      verify(cite.call).called(1);
      verify(action.call).called(1);
    });

    testWidgets('relation chip shows the core type label', (tester) async {
      await _pumpSingle(
        tester,
        const RelationChip(
          type: RelationType.related,
          label: 'Acme Logistics',
          typeLabel: 'works at',
          mentionOf: NodeKind.company,
        ),
      );
      expect(find.text('works at'), findsOneWidget);
      expect(find.bySemanticsLabel('works at: Acme Logistics'), findsOneWidget);
    });

    testWidgets('non-interactive chips expose no button semantics', (
      tester,
    ) async {
      await _pumpSingle(
        tester,
        const RelationChip(type: RelationType.related, label: 'Churn notes'),
      );
      final data = tester
          .getSemantics(find.bySemanticsLabel('related: Churn notes'))
          .getSemanticsData();
      expect(data.flagsCollection.isButton, isFalse);
      expect(tester.getSize(find.byType(RelationChip)).height, lessThan(48));
    });

    testWidgets('pointer platforms use 32 px minimum targets', (tester) async {
      setWindowSize(tester, const Size(800, 600));
      await tester.pumpWidget(
        MaterialApp(
          theme: StrataTheme.light(platform: TargetPlatform.macOS),
          localizationsDelegates: StrataLocalizations.localizationsDelegates,
          home: Scaffold(
            body: Center(
              child: StrataTapTarget(
                semanticLabel: 'x',
                onTap: () {},
                child: const SizedBox(width: 10, height: 10),
              ),
            ),
          ),
        ),
      );
      expect(tester.getSize(find.byType(StrataTapTarget)), const Size(32, 32));
    });

    testWidgets('avatar shows the core initials, or the icon', (tester) async {
      await _pumpSingle(
        tester,
        const Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            StrataAvatar(initials: 'AS'),
            StrataAvatar(initials: 'أس', square: true, size: 56),
            StrataAvatar(initials: ''),
          ],
        ),
      );
      expect(find.text('AS'), findsOneWidget);
      expect(find.text('أس'), findsOneWidget);
      expect(find.byIcon(Icons.person_outline), findsOneWidget);
      expect(
        tester.getSize(find.byType(StrataAvatar).at(1)),
        const Size(56, 56),
      );
      final square = tester.widget<Container>(
        find.descendant(
          of: find.byType(StrataAvatar).at(1),
          matching: find.byType(Container),
        ),
      );
      expect((square.decoration! as BoxDecoration).shape, BoxShape.rectangle);
      final handle = tester.ensureSemantics();
      expect(find.bySemanticsLabel('AS'), findsNothing);
      handle.dispose();
    });

    testWidgets('highlighted text marks the core ranges', (tester) async {
      await _pumpSingle(
        tester,
        const StrataHighlightedText(
          'Pricing experiments and pricing',
          highlights: [
            TextRange(start: 0, end: 7),
            TextRange(start: 24, end: 99),
          ],
        ),
      );
      final rich = tester.widget<Text>(find.byType(Text)).textSpan! as TextSpan;
      final parts = [
        for (final span in rich.children!.cast<TextSpan>())
          (span.text, span.style?.backgroundColor != null),
      ];
      expect(parts, [
        ('Pricing', true),
        (' experiments and ', false),
        ('pricing', true),
      ]);
    });

    test('AI tag text uses two decimals', () {
      expect(RelationChip.aiTagText('AI', 0.8), 'AI · 0.80');
      expect(RelationChip.aiTagText('AI', 0.8249), 'AI · 0.82');
    });

    test('sync status maps to tone and icon', () {
      expect(
        SyncPill.toneOf(const SyncSynced(lastSync: '14:32')),
        StatusTone.success,
      );
      expect(SyncPill.toneOf(const SyncOffline(queued: 3)), StatusTone.warning);
      expect(
        SyncPill.toneOf(const SyncInProgress(done: 1, total: 2)),
        StatusTone.info,
      );
      expect(SyncPill.toneOf(const SyncConflict(count: 1)), StatusTone.danger);
      expect({
        for (final s in const <SyncStatus>[
          SyncSynced(lastSync: ''),
          SyncOffline(queued: 0),
          SyncInProgress(done: 0, total: 0),
          SyncConflict(count: 0),
        ])
          SyncPill.iconOf(s),
      }, hasLength(4));
    });
  });
}
