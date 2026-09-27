import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Documents and places fixtures for this package's tests.
abstract final class DocFixtures {
  /// "Car licence": checked out, with Shawket.
  static final EntityScreen carLicence = EntityScreen(
    id: 'd-car-licence',
    kind: EntityPageKind.document,
    document: DocumentView(
      id: 'd-car-licence',
      title: 'Car licence',
      aliases: const ['رخصة العربية'],
      docType: 'licence',
      copy: 'original',
      status: 'checked-out',
      location: const [
        EntityRef(id: 'pl-home', title: 'Home'),
        EntityRef(id: 'pl-home-desk', title: 'Desk drawer'),
      ],
      holder: const EntityRef(id: 'p-shawket', title: 'Shawket'),
      custody: [
        CustodyItem(
          date: DateTime.utc(2026, 9, 25),
          kind: 'handed-to',
          person: const EntityRef(id: 'p-shawket', title: 'Shawket'),
          citations: const [StrataFixtures.citation],
        ),
      ],
      copies: const [],
      concerns: const [],
    ),
  );

  /// A document without custody, location or copies.
  static final EntityScreen bareDocument = EntityScreen(
    id: 'd-bare',
    kind: EntityPageKind.document,
    document: const DocumentView(
      id: 'd-bare',
      title: 'Title deed — Nasr City office',
      aliases: [],
      location: [],
      custody: [],
      copies: [],
      concerns: [],
    ),
  );

  /// "Safe — Nasr City office", nested in the office, nothing inside.
  static final EntityScreen safe = EntityScreen(
    id: 'pl-nasr-city-safe',
    kind: EntityPageKind.place,
    place: PlaceView(
      id: 'pl-nasr-city-safe',
      title: 'Safe — Nasr City office',
      aliases: const ['الخزنة — مكتب مدينة نصر'],
      breadcrumb: const [StrataFixtures.nasrCityOfficeRef],
      subPlaces: const [],
      documents: const [],
      recentMovements: const [],
    ),
  );

  /// An unknown ID.
  static const EntityScreen notFound = EntityScreen(
    id: 'x-gone',
    kind: EntityPageKind.notFound,
  );
}
