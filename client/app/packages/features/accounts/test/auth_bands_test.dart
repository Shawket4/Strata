import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';

/// The Android gesture bar's inset in these tests.
const double _inset = 24;

StrataBandsPainter _painterOf(WidgetTester tester, Finder bands) =>
    tester
            .widget<CustomPaint>(
              find.descendant(of: bands, matching: find.byType(CustomPaint)),
            )
            .painter!
        as StrataBandsPainter;

void main() {
  // Bands on phones without the sand seam (owner decision 2026-09-28); the
  // seam stays on medium and expanded. On compact and medium the bands run
  // to the bottom edge, under the system inset, at full depth above it.
  final screens = <String, (Widget, SessionState)>{
    'sign in': (const SignInScreen(), AccountFixtures.signedOut),
    'sign up': (SignUpScreen(onBack: () {}), StrataFixtures.sessionSignedOut),
    'approval': (const PendingApprovalScreen(), AccountFixtures.waiting),
  };
  for (final MapEntry(key: name, value: (screen, session)) in screens.entries) {
    group('$name bands', () {
      for (final v in variants()) {
        testWidgets('$v', (tester) async {
          tester.view.padding = const FakeViewPadding(bottom: _inset);
          addTearDown(tester.view.resetPadding);
          await pumpVariant(
            tester,
            v,
            screen,
            fake: FakeCoreApi()..session.add(session),
          );
          final bands = find.byType(StrataBands);
          expect(bands, findsOneWidget);
          final painter = _painterOf(tester, bands);
          final box = tester.getRect(bands);
          switch (v.sizeClass) {
            case SizeClass.compact:
              expect((painter.showSeam, painter.bleed), (false, _inset));
              expect(box.height, 72 + _inset);
              expect(box.bottom, v.size.height);
            case SizeClass.medium:
              expect((painter.showSeam, painter.bleed), (true, _inset));
              expect(box.height, 96 + _inset);
              expect(box.bottom, v.size.height);
            case SizeClass.expanded:
              // The brand panel's bands sit inside the safe area.
              expect(find.byType(BrandPanel), findsOneWidget);
              expect((painter.showSeam, painter.bleed), (true, 0.0));
              expect(box.height, 240);
              expect(box.bottom, v.size.height - _inset);
          }
          expect(painter.seam, tester.element(bands).strataColors.sand);
          expectNoErrors(tester);
        });
      }
    });
  }
}
