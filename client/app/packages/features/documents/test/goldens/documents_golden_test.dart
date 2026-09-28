import 'package:flutter_test/flutter_test.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';

FakeCoreApi _with(EntityScreen screen) => FakeCoreApi()
  ..placeOptionsAnswer.returns(StrataFixtures.placeOptions)
  ..entity[screen.id].add(screen);

/// Document and place pages × size class × theme × direction (1.0; compact
/// also 2.0), plus key states.
void main() {
  setUpAll(loadStrataFonts);

  group('document goldens', () {
    screenGoldens(
      'document',
      (v) => goldenFrame(
        v,
        DocumentScreen(
          'd-watanya-contract',
          onBack: () {},
          onOpenEntity: (_) {},
          onOpenNote: (_, _) {},
        ),
        fake: _with(StrataFixtures.entityScreenDocument)
          ..localGraph[('d-watanya-contract', 1)].add(DocFixtures.watanyaGraph),
        scaffold: true,
      ),
    );
    screenGoldens(
      'document_checked_out',
      (v) => goldenFrame(
        v,
        const DocumentScreen('d-car-licence'),
        fake: _with(DocFixtures.carLicence),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
    screenGoldens(
      'document_not_found',
      (v) => goldenFrame(
        v,
        const DocumentScreen('x-gone'),
        fake: _with(DocFixtures.notFound),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });

  group('place goldens', () {
    screenGoldens(
      'place',
      (v) => goldenFrame(
        v,
        PlaceScreen(
          'pl-nasr-city-office',
          onBack: () {},
          onOpenEntity: (_) {},
          onOpenNote: (_, _) {},
        ),
        fake: _with(StrataFixtures.entityScreenPlace),
        scaffold: true,
      ),
    );
    screenGoldens(
      'place_empty',
      (v) => goldenFrame(
        v,
        const PlaceScreen('pl-nasr-city-safe'),
        fake: _with(DocFixtures.safe),
        scaffold: true,
      ),
      cells: goldenVariants(wide: false),
    );
  });
}
