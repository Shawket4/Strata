import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

import 'helpers/matrix.dart';

void main() {
  for (final v in variants()) {
    testWidgets('documents list $v', (tester) async {
      final fake = FakeCoreApi();
      fake.directory[(DirectoryTab.documents, '')].add(
        const DirectoryView(
          tab: DirectoryTab.documents,
          query: '',
          items: [
            DirectoryItem(
              id: 'd-watanya-contract',
              title: 'Watanya contract',
              subtitle: 'Stored · Nasr City office › Safe',
              aliases: ['عقد وطنية'],
            ),
          ],
          counts: StrataFixtures.directoryCounts,
        ),
      );
      final opened = <String>[];
      await pumpVariant(
        tester,
        v,
        DocumentsScreen(onOpenEntity: opened.add),
        fake,
      );
      expectNoErrors(tester);
      expect(
        fake.calls,
        contains(
          const CoreCall('watchDirectory', {
            'tab': DirectoryTab.documents,
            'query': '',
          }),
        ),
      );
      if (v.textScale == 1) await expectAccessible(tester);
      await tester.tap(find.text('Watanya contract'));
      expect(opened, ['d-watanya-contract']);
    });
  }
}
