import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'helpers/fixtures.dart';
import 'helpers/hosts.dart';

/// Every line of [value]'s paragraph, as laid out, in order.
List<String> _lines(WidgetTester tester, String value) {
  final paragraph = tester.renderObject<RenderParagraph>(find.text(value));
  final painter = TextPainter(
    text: paragraph.text,
    textAlign: paragraph.textAlign,
    textDirection: paragraph.textDirection,
    textScaler: paragraph.textScaler,
  )..layout(maxWidth: paragraph.size.width);
  addTearDown(painter.dispose);
  final lines = <String>[];
  var offset = 0;
  while (offset < value.length) {
    final end = painter.getLineBoundary(TextPosition(offset: offset)).end;
    lines.add(value.substring(offset, end).trim());
    offset = end;
    while (offset < value.length && value[offset] == ' ') {
      offset++;
    }
  }
  return lines;
}

/// Fails when [value] wraps anywhere but between words.
void _expectWordWrapped(WidgetTester tester, String value) {
  final words = value.split(' ').toSet();
  for (final line in _lines(tester, value)) {
    expect(
      line.split(' ').every(words.contains),
      isTrue,
      reason: '"$value" breaks mid-word: "$line"',
    );
  }
}

void main() {
  // Real glyphs: wrapping depends on the bundled Cairo / Quicksand metrics.
  setUpAll(loadStrataFonts);

  group('account sheet rows wrap between words', () {
    for (final v in variants(sizes: goldenSizes)) {
      testWidgets('$v', (tester) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          const AccountHost(),
          fake: FakeCoreApi()
            ..session.add(AccountFixtures.active)
            ..settings.add(StrataFixtures.settingsView),
        );
        await tester.tap(find.text(AccountHost.openLabel));
        await settle(tester);

        final pending = l10n.pendingCount(count: 2);
        for (final value in [
          l10n.devices,
          l10n.adminUsers,
          pending,
          l10n.signOut,
        ]) {
          _expectWordWrapped(tester, value);
        }

        // The pending count sits beside "Manage users" when both fit on one
        // line and moves below it otherwise, never squeezing the label.
        final label = tester.getRect(find.text(l10n.adminUsers));
        final count = tester.getRect(find.text(pending));
        final besideLabel =
            count.center.dy > label.top &&
            count.center.dy < label.bottom &&
            (v.rtl ? count.right <= label.left : count.left >= label.right);
        final belowLabel = count.top >= label.bottom;
        expect(besideLabel || belowLabel, isTrue, reason: '$label $count');
        if (v.sizeClass == SizeClass.compact && v.textScale == 2) {
          expect(belowLabel, isTrue, reason: 'too narrow for one line');
        }
        if (v.textScale == 1) {
          expect(besideLabel, isTrue, reason: 'fits on one line');
          expect(_lines(tester, l10n.adminUsers), hasLength(1));
        }
        expectNoErrors(tester);
      });
    }
  });
}
