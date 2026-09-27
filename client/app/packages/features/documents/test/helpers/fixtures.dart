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
  static const EntityScreen bareDocument = EntityScreen(
    id: 'd-bare',
    kind: EntityPageKind.document,
    document: DocumentView(
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
  static const EntityScreen safe = EntityScreen(
    id: 'pl-nasr-city-safe',
    kind: EntityPageKind.place,
    place: PlaceView(
      id: 'pl-nasr-city-safe',
      title: 'Safe — Nasr City office',
      aliases: ['الخزنة — مكتب مدينة نصر'],
      breadcrumb: [StrataFixtures.nasrCityOfficeRef],
      subPlaces: [],
      documents: [],
      recentMovements: [],
    ),
  );

  /// An unknown ID.
  static const EntityScreen notFound = EntityScreen(
    id: 'x-gone',
    kind: EntityPageKind.notFound,
  );

  /// The Watanya contract's neighbourhood.
  static const LocalGraphView watanyaGraph = LocalGraphView(
    center: 'd-watanya-contract',
    found: true,
    depth: 1,
    nodes: [
      GraphNode(
        id: 'd-watanya-contract',
        title: 'Watanya contract',
        kind: 'document',
        depth: 0,
        degree: 4,
        x: 0,
        y: 0,
      ),
      GraphNode(
        id: 'pl-nasr-city-safe',
        title: 'Safe — Nasr City office',
        kind: 'place',
        depth: 1,
        degree: 2,
        x: 120,
        y: -40,
      ),
      GraphNode(
        id: 'p-shady',
        title: 'Shady',
        kind: 'person',
        depth: 1,
        degree: 2,
        x: -100,
        y: 60,
      ),
      GraphNode(
        id: 'c-watanya',
        title: 'Watanya',
        kind: 'company',
        depth: 1,
        degree: 3,
        x: 40,
        y: 110,
      ),
    ],
    edges: [
      GraphEdge(
        src: 'd-watanya-contract',
        dst: 'pl-nasr-city-safe',
        kind: 'custody:location',
      ),
      GraphEdge(
        src: 'd-watanya-contract',
        dst: 'p-shady',
        kind: 'custody:last-holder',
      ),
      GraphEdge(src: 'd-watanya-contract', dst: 'c-watanya', kind: 'mention'),
    ],
  );
}
