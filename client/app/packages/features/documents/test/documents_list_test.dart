import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

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
              kind: '',
              titleDir: TextDir.ltr,
              initials: '',
              mentionCount: 0,
              tags: [],
              location: [],
              expiringSoon: false,
              breadcrumb: [],
              documentCount: 0,
              hasOpenItems: false,
            ),
          ],
          counts: StrataFixtures.directoryCounts,
          filter: DirectoryFilter(
            tags: [],
            expiring: false,
            hasOpenItems: false,
          ),
          sort: DirectorySort.name,
          filterOptions: [],
          sections: [],
          suggestions: [],
          expiringCount: 0,
        ),
      );
      final opened = <String>[];
      await pumpVariant(
        tester,
        v,
        DocumentsScreen(onOpenEntity: opened.add),
        fake: fake,
        scaffold: true,
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
      expect(find.text('Stored · Nasr City office › Safe'), findsOneWidget);
      await expectAccessible(tester, contrast: v.textScale == 1);
      await tester.tap(find.text('Watanya contract'));
      expect(opened, ['d-watanya-contract']);
    });
  }
}
