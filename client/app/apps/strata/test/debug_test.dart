import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/strata.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/boot.dart';

void main() {
  testWidgets('debug', (tester) async {
    final fake = FakeCoreApi()..notesList[''].add(StrataFixtures.notesListView);
    final app = await boot(tester, fake: fake, size: StrataTestSizes.expanded);
    await go(tester, app, const SettingsRoute().location);
    final h = tester.ensureSemantics();
    final node = tester.getSemantics(find.text('clients').first);
    debugPrint('node "${node.label}" ${node.getSemanticsData()}');
    h.dispose();
  });
}
