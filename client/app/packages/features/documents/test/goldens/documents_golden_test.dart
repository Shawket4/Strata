import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/matrix.dart';

FakeCoreApi _with(EntityScreen screen) =>
    FakeCoreApi()..entity[screen.id].add(screen);

/// Document and place pages × size class × theme × direction (1.0; compact
/// also 2.0), plus key states.
void main() {
  setUpAll(loadStrataFonts);

  group('document goldens', () {
    screenGoldens(
      'document',
      () => DocumentScreen(
        'd-watanya-contract',
        onBack: () {},
        onOpenEntity: (_) {},
        onOpenNote: (_, _) {},
      ),
      () => _with(StrataFixtures.entityScreenDocument)
        ..localGraph[('d-watanya-contract', 1)].add(DocFixtures.watanyaGraph),
    );
    screenGoldens(
      'document_checked_out',
      () => const DocumentScreen('d-car-licence'),
      () => _with(DocFixtures.carLicence),
      cells: goldenVariants(wide: false),
    );
    screenGoldens(
      'document_not_found',
      () => const DocumentScreen('x-gone'),
      () => _with(DocFixtures.notFound),
      cells: goldenVariants(wide: false),
    );
  });

  group('place goldens', () {
    screenGoldens(
      'place',
      () => PlaceScreen(
        'pl-nasr-city-office',
        onBack: () {},
        onOpenEntity: (_) {},
        onOpenNote: (_, _) {},
      ),
      () => _with(StrataFixtures.entityScreenPlace),
    );
    screenGoldens(
      'place_empty',
      () => const PlaceScreen('pl-nasr-city-safe'),
      () => _with(DocFixtures.safe),
      cells: goldenVariants(wide: false),
    );
  });
}
