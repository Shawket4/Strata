import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

import '../helpers/harness.dart';

class _Panes extends StatelessWidget {
  const new({
    this.open = false,
    this.onClosed,
    this.showDetail = false,
    this.withContext = true,
  });

  final bool open;
  final VoidCallback? onClosed;
  final bool showDetail;
  final bool withContext;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Scaffold(
      body: StrataPanes(
        list: const ColoredBox(
          key: Key('list'),
          color: Color(0x00000000),
          child: Center(child: Text('Pricing experiments')),
        ),
        detail: const Center(key: Key('detail'), child: Text('Churn notes')),
        contextPanel: withContext
            ? const Center(key: Key('context'), child: Text('Backlinks'))
            : null,
        contextPanelOpen: open,
        onContextPanelClosed: onClosed,
        contextPanelLabel: l10n.contextPanelLabel,
        dismissLabel: l10n.actionClose,
        showDetailOnCompact: showDetail,
      ),
    );
  }
}

void main() {
  group('StrataPanes structure', () {
    for (final v in variants()) {
      testWidgets('$v', (tester) async {
        await pumpVariant(tester, v, const _Panes(open: true));
        final l10n = await StrataLocalizations.delegate.load(v.locale);
        final list = find.byKey(const Key('list'));
        final detail = find.byKey(const Key('detail'));
        final context = find.byKey(const Key('context'));
        switch (v.sizeClass) {
          case SizeClass.compact:
            expect(list, findsOneWidget);
            expect(detail, findsNothing);
            expect(context, findsNothing);
          case SizeClass.medium:
            expect(tester.getSize(list).width, StrataLayout.listPaneWidth);
            expect(detail, findsOneWidget);
            expect(find.byType(ModalBarrier), findsWidgets);
            final panel = find.bySemanticsLabel(l10n.contextPanelLabel);
            expect(panel, findsOneWidget);
            final rect = tester.getRect(
              find.ancestor(of: context, matching: find.byType(DecoratedBox)),
            );
            expect(rect.width, StrataLayout.contextPanelWidth);
            // The drawer slides in from the end edge.
            if (v.direction == TextDirection.ltr) {
              expect(rect.right, v.size.width);
            } else {
              expect(rect.left, 0);
            }
          case SizeClass.expanded:
            expect(tester.getSize(list).width, StrataLayout.listPaneWidth);
            final panel = tester.getRect(
              find.ancestor(of: context, matching: find.byType(DecoratedBox)),
            );
            expect(panel.width, StrataLayout.contextPanelWidth);
            final main = tester.getRect(detail);
            expect(
              main.width,
              v.size.width -
                  StrataLayout.listPaneWidth -
                  StrataLayout.contextPanelWidth -
                  2,
            );
            if (v.direction == TextDirection.ltr) {
              expect(tester.getTopLeft(list).dx, 0);
              expect(panel.right, v.size.width);
            } else {
              expect(tester.getTopRight(list).dx, v.size.width);
              expect(panel.left, 0);
            }
        }
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });
    }
  });

  group('StrataPanes behaviour', () {
    final base = variants(textScales: const [1]).where(
      (v) => v.brightness == Brightness.light && v.locale.languageCode == 'en',
    );
    MatrixVariant at(SizeClass c) => base.firstWhere((v) => v.sizeClass == c);

    testWidgets('compact shows the detail pane when asked', (tester) async {
      await pumpVariant(
        tester,
        at(SizeClass.compact),
        const _Panes(showDetail: true),
      );
      expect(find.text('Churn notes'), findsOneWidget);
      expect(find.text('Pricing experiments'), findsNothing);
    });

    testWidgets('medium hides the context drawer until opened', (tester) async {
      await pumpVariant(tester, at(SizeClass.medium), const _Panes());
      expect(find.text('Backlinks'), findsNothing);
      expect(find.text('Churn notes'), findsOneWidget);
    });

    testWidgets('tapping the scrim closes the medium drawer', (tester) async {
      var closed = 0;
      await pumpVariant(
        tester,
        at(SizeClass.medium),
        _Panes(open: true, onClosed: () => closed++),
      );
      await tester.tapAt(const Offset(500, 300));
      expect(closed, 1);
    });

    testWidgets('expanded without a context panel uses two panes', (
      tester,
    ) async {
      await pumpVariant(
        tester,
        at(SizeClass.expanded),
        const _Panes(withContext: false),
      );
      expect(
        tester.getSize(find.byKey(const Key('detail'))).width,
        1440 - StrataLayout.listPaneWidth - 1,
      );
    });

    testWidgets('compact context opens as a bottom sheet', (tester) async {
      await pumpVariant(
        tester,
        at(SizeClass.compact),
        Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showStrataContextSheet<void>(
                context: context,
                builder: (_) => const SizedBox(
                  height: 200,
                  child: Center(child: Text('Backlinks')),
                ),
              ),
              child: const Text('Context'),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Context'));
      await tester.pumpAndSettle();
      expect(find.byType(BottomSheet), findsOneWidget);
      expect(find.text('Backlinks'), findsOneWidget);
      final sheet = tester.widget<BottomSheet>(find.byType(BottomSheet));
      expect(sheet.showDragHandle, isTrue);
      final theme = Theme.of(tester.element(find.byType(BottomSheet)));
      expect(
        theme.bottomSheetTheme.shape,
        const RoundedRectangleBorder(borderRadius: StrataRadii.sheetTopRadius),
      );
    });
  });
}
