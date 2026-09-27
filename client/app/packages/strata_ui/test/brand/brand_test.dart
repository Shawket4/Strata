import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';

import '../helpers/harness.dart';

class _Brand extends StatelessWidget {
  const _Brand();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          StrataWordmark(),
          Row(
            children: [
              StrataSymbol(size: 16),
              StrataSymbol(),
              StrataSymbol(size: 48, semanticLabel: 'Strata symbol'),
            ],
          ),
          SizedBox(height: 120, child: StrataBands()),
        ],
      ),
    );
  }
}

void main() {
  group('StrataSymbolPainter geometry', () {
    test('standard path spans the spec coordinates on the 64 grid', () {
      final bounds = StrataSymbolPainter.pathOf(
        StrataSymbolForm.standard,
      ).getBounds();
      // M50 13 … H14; the bowls reach x = 24 - 9.5 and x = 40 + 9.5.
      expect(bounds.left, closeTo(14, 0.01));
      expect(bounds.top, closeTo(13, 0.01));
      expect(bounds.right, closeTo(50, 0.01));
      expect(bounds.bottom, closeTo(51, 0.01));
      expect(StrataSymbolPainter.strokeWidthOf(StrataSymbolForm.standard), 11);
    });

    test('favicon path uses the heavier small-size drawing', () {
      final bounds = StrataSymbolPainter.pathOf(
        StrataSymbolForm.favicon,
      ).getBounds();
      expect(bounds.left, closeTo(15, 0.01));
      expect(bounds.top, closeTo(14, 0.01));
      expect(bounds.right, closeTo(49, 0.01));
      expect(bounds.bottom, closeTo(50, 0.01));
      expect(StrataSymbolPainter.strokeWidthOf(StrataSymbolForm.favicon), 14);
    });

    test('the top bowl opens left and the bottom bowl right (an S)', () {
      final path = StrataSymbolPainter.pathOf(StrataSymbolForm.standard);
      // Upper bowl: centre (24, 22.5), leftmost point x = 14.5.
      expect(path.contains(const Offset(14.8, 22.5)), isFalse);
      final metrics = path.computeMetrics().single;
      final points = [
        for (var d = 0.0; d <= metrics.length; d += 0.5)
          metrics.getTangentForOffset(d)!.position,
      ];
      final upper = points.where((p) => p.dy > 14 && p.dy < 31);
      final lower = points.where((p) => p.dy > 33 && p.dy < 50);
      expect(upper.map((p) => p.dx).reduce((a, b) => a < b ? a : b), lessThan(15));
      expect(lower.map((p) => p.dx).reduce((a, b) => a > b ? a : b), greaterThan(49));
    });

    test('form defaults to favicon at 32 px and below', () {
      expect(StrataSymbol.formFor(16), StrataSymbolForm.favicon);
      expect(StrataSymbol.formFor(32), StrataSymbolForm.favicon);
      expect(StrataSymbol.formFor(33), StrataSymbolForm.standard);
    });

    test('painter repaints only on colour or form changes', () {
      const a = StrataSymbolPainter(color: StrataPalette.tide);
      expect(a.shouldRepaint(const StrataSymbolPainter(color: StrataPalette.tide)), isFalse);
      expect(a.shouldRepaint(const StrataSymbolPainter(color: StrataPalette.mist)), isTrue);
      expect(
        a.shouldRepaint(
          const StrataSymbolPainter(
            color: StrataPalette.tide,
            form: StrataSymbolForm.favicon,
          ),
        ),
        isTrue,
      );
    });
  });

  group('StrataBandsPainter', () {
    test('bands step one tone apart from the base', () {
      const painter = StrataBandsPainter(
        depths: StrataBands.defaultDepths,
        seamAfter: 2,
        base: StrataPalette.mist,
        tone: StrataPalette.tide,
        seam: StrataPalette.sand,
      );
      expect(painter.bandColor(0), StrataPalette.mist);
      final lum = [
        for (var i = 0; i < 5; i++) painter.bandColor(i).computeLuminance(),
      ];
      for (var i = 1; i < lum.length; i++) {
        expect(lum[i], lessThan(lum[i - 1]));
      }
      expect(painter.shouldRepaint(painter), isFalse);
    });
  });

  group('brand widgets', () {
    for (final v in variants()) {
      testWidgets('$v', (tester) async {
        await pumpVariant(tester, v, const _Brand());
        final symbols = tester.widgetList<StrataSymbol>(
          find.byType(StrataSymbol),
        );
        expect(symbols, hasLength(4));
        for (final symbol in symbols) {
          final paint = tester.widget<CustomPaint>(
            find.descendant(
              of: find.byWidget(symbol),
              matching: find.byType(CustomPaint),
            ),
          );
          final painter = paint.painter! as StrataSymbolPainter;
          expect(painter.color, StrataPalette.tide);
          expect(painter.form, StrataSymbol.formFor(symbol.size));
        }
        expect(find.bySemanticsLabel('Strata'), findsOneWidget);
        expect(find.bySemanticsLabel('Strata symbol'), findsOneWidget);
        final wordmark = tester.widget<Text>(find.text(StrataWordmark.text));
        expect(wordmark.style?.fontFamily, 'packages/strata_ui/Quicksand');
        expect(wordmark.style?.color, v.theme.extension<StrataColors>()!.text);
        // The Latin lockup never mirrors: symbol stays left of the text.
        expect(
          tester.getCenter(find.byType(StrataSymbol).first).dx,
          lessThan(tester.getCenter(find.text(StrataWordmark.text)).dx),
        );
        final bands = tester.widget<CustomPaint>(
          find.descendant(
            of: find.byType(StrataBands),
            matching: find.byType(CustomPaint),
          ),
        );
        final bandsPainter = bands.painter! as StrataBandsPainter;
        expect(bandsPainter.seam, StrataPalette.sand);
        expect(
          bandsPainter.base,
          v.theme.extension<StrataColors>()!.background,
        );
        expectNoRenderErrors(tester);
        await expectAccessible(tester);
      });
    }
  });
}
