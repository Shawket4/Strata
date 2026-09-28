import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';

import '../helpers/harness.dart';

class _Brand extends StatelessWidget {
  const new();

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
      final bounds = StrataSymbolPainter.pathOf(StrataSymbolForm.standard)
          .getBounds();
      // M50 13 … H14; the bowls reach x = 24 - 9.5 and x = 40 + 9.5.
      expect(bounds.left, closeTo(14, 0.01));
      expect(bounds.top, closeTo(13, 0.01));
      expect(bounds.right, closeTo(50, 0.01));
      expect(bounds.bottom, closeTo(51, 0.01));
      expect(StrataSymbolPainter.strokeWidthOf(StrataSymbolForm.standard), 11);
    });

    test('favicon path uses the heavier small-size drawing', () {
      final bounds = StrataSymbolPainter.pathOf(StrataSymbolForm.favicon)
          .getBounds();
      expect(bounds.left, closeTo(15, 0.01));
      expect(bounds.top, closeTo(14, 0.01));
      expect(bounds.right, closeTo(49, 0.01));
      expect(bounds.bottom, closeTo(50, 0.01));
      expect(StrataSymbolPainter.strokeWidthOf(StrataSymbolForm.favicon), 14);
    });

    test('the top bowl opens left and the bottom bowl right (an S)', () {
      final path = StrataSymbolPainter.pathOf(StrataSymbolForm.standard);
      // Upper bowl: centre (24, 22.5), leftmost point x = 14.5; lower bowl:
      // centre (40, 41.5), rightmost point x = 49.5.
      final metrics = path.computeMetrics().single;
      final points = [
        for (var d = 0.0; d <= metrics.length; d += 0.5)
          metrics.getTangentForOffset(d)!.position,
      ];
      final upper = points.where((p) => p.dy > 14 && p.dy < 31);
      final lower = points.where((p) => p.dy > 33 && p.dy < 50);
      expect(
        upper.map((p) => p.dx).reduce((a, b) => a < b ? a : b),
        lessThan(15),
      );
      expect(
        lower.map((p) => p.dx).reduce((a, b) => a > b ? a : b),
        greaterThan(49),
      );
    });

    test('form defaults to favicon at 32 px and below', () {
      expect(StrataSymbol.formFor(16), StrataSymbolForm.favicon);
      expect(StrataSymbol.formFor(32), StrataSymbolForm.favicon);
      expect(StrataSymbol.formFor(33), StrataSymbolForm.standard);
    });

    test('painter repaints only on colour or form changes', () {
      const a = StrataSymbolPainter(color: StrataPalette.tide);
      expect(
        a.shouldRepaint(const StrataSymbolPainter(color: StrataPalette.tide)),
        isFalse,
      );
      expect(
        a.shouldRepaint(const StrataSymbolPainter(color: StrataPalette.mist)),
        isTrue,
      );
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

  group('StrataBands seam and bleed', () {
    // Depths 1:1:2 over 100 px: bands at 0–25, 25–50, 50–100; the seam
    // (2 px, sand) is centred on the end of band 1.
    const depths = [1.0, 1.0, 2.0];

    Widget bands({bool? showSeam, double bleed = 0}) => Align(
      alignment: Alignment.topLeft,
      child: SizedBox(
        width: 200,
        height: 100,
        child: showSeam == null
            ? StrataBands(depths: depths, seamAfter: 1, bleed: bleed)
            : StrataBands(
                depths: depths,
                seamAfter: 1,
                showSeam: showSeam,
                bleed: bleed,
              ),
      ),
    );

    RenderObject paintOf(WidgetTester tester) => tester.renderObject(
      find.descendant(
        of: find.byType(StrataBands),
        matching: find.byType(CustomPaint),
      ),
    );

    for (final v in variants()) {
      testWidgets('the seam is drawn by default $v', (tester) async {
        await pumpVariant(tester, v, bands());
        final colors = v.theme.extension<StrataColors>()!;
        final painter =
            tester
                    .widget<CustomPaint>(
                      find.descendant(
                        of: find.byType(StrataBands),
                        matching: find.byType(CustomPaint),
                      ),
                    )
                    .painter!
                as StrataBandsPainter;
        expect((painter.showSeam, painter.bleed), (true, 0.0));
        expect(
          paintOf(tester),
          paints
            ..rect(rect: const Rect.fromLTWH(0, 0, 200, 25))
            ..rect(rect: const Rect.fromLTWH(0, 25, 200, 25))
            ..rect(rect: const Rect.fromLTWH(0, 50, 200, 50))
            ..rect(
              rect: const Rect.fromLTWH(0, 49, 200, 2),
              color: colors.sand,
            ),
        );
        expect(paintOf(tester), paintsExactlyCountTimes(#drawRect, 4));
        expectNoRenderErrors(tester);
      });

      testWidgets('showSeam: false keeps the bands without the seam $v', (
        tester,
      ) async {
        await pumpVariant(tester, v, bands(showSeam: false));
        final painter =
            tester
                    .widget<CustomPaint>(
                      find.descendant(
                        of: find.byType(StrataBands),
                        matching: find.byType(CustomPaint),
                      ),
                    )
                    .painter!
                as StrataBandsPainter;
        expect(painter.showSeam, isFalse);
        expect(
          paintOf(tester),
          paints
            ..rect(rect: const Rect.fromLTWH(0, 0, 200, 25))
            ..rect(rect: const Rect.fromLTWH(0, 25, 200, 25))
            ..rect(rect: const Rect.fromLTWH(0, 50, 200, 50)),
        );
        expect(paintOf(tester), paintsExactlyCountTimes(#drawRect, 3));
        expectNoRenderErrors(tester);
      });

      testWidgets('bleed continues the last band below the bands $v', (
        tester,
      ) async {
        await pumpVariant(tester, v, bands(showSeam: false, bleed: 20));
        // The bands keep 1:1:2 over the top 80 px; the last band runs on
        // through the 20 px bleed.
        expect(
          paintOf(tester),
          paints
            ..rect(rect: const Rect.fromLTWH(0, 0, 200, 20))
            ..rect(rect: const Rect.fromLTWH(0, 20, 200, 20))
            ..rect(rect: const Rect.fromLTWH(0, 40, 200, 60)),
        );
        expect(paintOf(tester), paintsExactlyCountTimes(#drawRect, 3));
        expectNoRenderErrors(tester);
      });
    }

    test('the seam flag and the bleed repaint', () {
      const base = StrataBandsPainter(
        depths: depths,
        seamAfter: 1,
        base: StrataPalette.mist,
        tone: StrataPalette.tide,
        seam: StrataPalette.sand,
      );
      const noSeam = StrataBandsPainter(
        depths: depths,
        seamAfter: 1,
        base: StrataPalette.mist,
        tone: StrataPalette.tide,
        seam: StrataPalette.sand,
        showSeam: false,
      );
      const bled = StrataBandsPainter(
        depths: depths,
        seamAfter: 1,
        base: StrataPalette.mist,
        tone: StrataPalette.tide,
        seam: StrataPalette.sand,
        bleed: 24,
      );
      expect(noSeam.shouldRepaint(base), isTrue);
      expect(bled.shouldRepaint(base), isTrue);
      expect(base.shouldRepaint(base), isFalse);
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
