import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/harness.dart';

void main() {
  for (final v in variants(sizes: {'medium': StrataTestSizes.medium}, textScales: const [1]).where((v) => v.brightness == Brightness.light)) {
    for (final sel in [Dest.settings, Dest.notes]) {
    testWidgets('probe $v $sel', (tester) async {
      await pumpVariant(tester, v, TestShell(selectedIndex: sel));
      final rv = tester.binding.renderViews.first;
      final layer = rv.debugLayer! as OffsetLayer;
      final bytes = await tester.binding.runAsync(() async {
        final img = await layer.toImage(rv.paintBounds);
        final d = await img.toByteData();
        return (d!, img.width);
      });
      for (final label in ['الملاحظات', 'الإعدادات', 'Notes', 'Settings']) {
        final f = find.text(label);
        if (f.evaluate().isEmpty) continue;
        final rb = f.evaluate().first.renderObject! as RenderBox;
        final r = MatrixUtils.transformRect(rb.getTransformTo(null), rb.paintBounds.inflate(4));
        final hist = <int, int>{};
        final (data, w) = bytes!;
        for (var x = r.left.floor(); x < r.right.ceil(); x++) {
          for (var y = r.top.floor(); y < r.bottom.ceil(); y++) {
            final p = data.getUint32((y * w + x) * 4);
            hist[p] = (hist[p] ?? 0) + 1;
          }
        }
        final top = hist.entries.toList()..sort((a, b) => b.value.compareTo(a.value));
        // ignore: avoid_print
        print('$label sel=$sel $r ${top.take(8).map((e) => '${e.key.toRadixString(16)}:${e.value}').join(' ')}');
        if (label == 'الإعدادات' || label == 'الملاحظات') {
          final (data, w) = bytes;
          for (var y = r.top.floor(); y < r.bottom.ceil(); y++) {
            final sb = StringBuffer();
            for (var x = r.left.floor(); x < r.right.ceil(); x++) {
              final p = data.getUint32((y * w + x) * 4);
              final g = (p >> 16) & 0xff;
              sb.write(g > 0xe0 ? '.' : g > 0xa0 ? '+' : '#');
            }
            // ignore: avoid_print
            print(sb);
          }
        }
      }
    });
    }
  }
}
