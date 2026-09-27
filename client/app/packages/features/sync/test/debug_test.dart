import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart';

import 'helpers/fixtures.dart';
import 'helpers/hosts.dart';
import 'helpers/matrix.dart';

void main() {
  testWidgets('debug', (tester) async {
    final v = matrix().firstWhere((v) => v.id == 'expanded_light_rtl_2x');
    final fake = FakeCoreApi()..session.add(StrataFixtures.sessionActive)..syncStatus.add(SyncFixtures.offline);
    await pumpVariant(tester, v, const SyncHost(), fake: fake);
    await tester.tap(find.text(SyncHost.openLabel));
    await settle(tester);
    debugPrint('popover ${tester.getRect(find.byType(SyncPopover))}');
    debugPrint('button ${tester.getRect(find.text(SyncHost.openLabel))}');
    for (final e in find.text('آخر مزامنة').evaluate()) {
      debugPrint('label ${tester.getRect(find.byWidget(e.widget))}');
    }
  });
}
