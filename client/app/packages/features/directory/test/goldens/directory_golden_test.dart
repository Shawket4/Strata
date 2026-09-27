import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_state/strata_state.dart' hide EntityScreen;
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

FakeCoreApi _fake() {
  final fake = FakeCoreApi();
  fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
  fake.directory[(DirectoryTab.documents, '')].add(DirFixtures.documents);
  fake.inbox.add(StrataFixtures.inboxView);
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
      () => DirectoryScreen(selectedId: 'p-ahmed-samir', onOpenEntity: (_) {}),
      _fake,
    );
    screenGoldens(
      'directory_documents',
      () => DirectoryScreen(
        initialTab: DirectoryTab.documents,
        selectedId: 'd-watanya-contract',
        onOpenEntity: (_) {},
      ),
      _fake,
    );
  });

  group('entity goldens', () {
    screenGoldens(
      'entity',
      () => EntityScreen(
        'p-ahmed-samir',
        onBack: () {},
        onOpenEntity: (_) {},
        onOpenNote: (_, _) {},
        onOpenMindMap: (_) {},
      ),
      _fake,
    );
    screenGoldens(
      'entity_offline_edits',
      () => const EntityScreen('p-ahmed-samir'),
      () =>
          FakeCoreApi()..entity['p-ahmed-samir'].add(DirFixtures.ahmedPending),
      cells: goldenVariants(wide: false),
    );
  });
}
