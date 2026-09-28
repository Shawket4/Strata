import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

FakeCoreApi _fake() {
  final fake = FakeCoreApi();
  fake
      .directoryFiltered[(
        DirectoryTab.people,
        '',
        emptyDirectoryFilter,
        DirectorySort.name,
      )]
      .add(DirFixtures.people);
  fake
      .directoryFiltered[(
        DirectoryTab.documents,
        '',
        emptyDirectoryFilter,
        DirectorySort.name,
      )]
      .add(DirFixtures.documents);
  fake.entity['p-ahmed-samir'].add(StrataFixtures.entityScreen);
  fake.entity['d-watanya-contract'].add(StrataFixtures.entityScreenDocument);
  fake.localGraph[('p-ahmed-samir', 1)].add(StrataFixtures.localGraphView);
  return fake;
}

/// Directory and entity pages × size class × theme × direction (1.0;
/// compact also 2.0).
void main() {
  setUpAll(loadStrataFonts);

  group('directory goldens', () {
    screenGoldens(
      'directory_people',
      (v) => goldenFrame(
        v,
        DirectoryScreen(selectedId: 'p-ahmed-samir', onOpenEntity: (_) {}),
        fake: _fake(),
        scaffold: true,
      ),
    );
    screenGoldens(
      'directory_documents',
      (v) => goldenFrame(
        v,
        DirectoryScreen(
          initialTab: DirectoryTab.documents,
          selectedId: 'd-watanya-contract',
          onOpenEntity: (_) {},
        ),
        fake: _fake(),
        scaffold: true,
      ),
    );
  });

  group('entity goldens', () {
    screenGoldens(
      'entity',
      (v) => goldenFrame(
        v,
        EntityPage(
          'p-ahmed-samir',
          onBack: () {},
          onOpenEntity: (_) {},
          onOpenNote: (_, _) {},
          onOpenMindMap: (_) {},
        ),
        fake: _fake(),
        scaffold: true,
      ),
    );
    screenGoldens(
      'entity_offline_edits',
      (v) => goldenFrame(
        v,
        const EntityPage('p-ahmed-samir'),
        fake: FakeCoreApi()
          ..entity['p-ahmed-samir'].add(DirFixtures.ahmedPending),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });
}
