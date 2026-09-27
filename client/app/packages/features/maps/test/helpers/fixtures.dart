import 'package:strata_state/strata_state.dart';

/// Map fixtures for this package's tests (sample content of
/// design/SCREEN_SPEC.md), in addition to `StrataFixtures`.
abstract final class MapFixtures {
  static GraphNode _n(
    String id,
    String title,
    String kind,
    double x,
    double y, {
    int degree = 3,
    int depth = 1,
    String? cluster,
  }) => GraphNode(
    id: id,
    title: title,
    kind: kind,
    depth: depth,
    clusterId: cluster,
    degree: degree,
    x: x,
    y: y,
  );

  static GraphEdge _e(
    String src,
    String dst,
    String kind, {
    double? confidence,
  }) => GraphEdge(
    src: src,
    dst: dst,
    kind: kind,
    by: confidence == null ? 'user' : 'ai',
    confidence: confidence,
  );

  /// "Pricing experiments" with its eight neighbours (MindMap artboards):
  /// every relation style, a concept, a person and a company.
  static final LocalGraphView pricingLocal = LocalGraphView(
    center: 'n-pricing-experiments',
    found: true,
    depth: 1,
    nodes: [
      _n(
        'n-pricing-experiments',
        'Pricing experiments',
        'note',
        0,
        0,
        degree: 8,
        depth: 0,
        cluster: 'k-pricing',
      ),
      _n('k-pricing', 'Pricing', 'concept', 0, -130, cluster: 'k-pricing'),
      _n('k-loyalty', 'Loyalty', 'concept', 120, -90, cluster: 'k-pricing'),
      _n(
        'n-subscription-tiers',
        'Subscription tiers',
        'note',
        150,
        20,
        cluster: 'k-pricing',
      ),
      _n(
        'n-churn-notes',
        'Churn notes',
        'note',
        100,
        120,
        cluster: 'k-pricing',
      ),
      _n(
        'n-discount-policy',
        'Discount policy',
        'note',
        -110,
        110,
        cluster: 'k-pricing',
      ),
      _n(
        'n-call-acme',
        'Call 2026-09-12 — Acme',
        'note',
        -160,
        10,
        cluster: 'k-clients',
      ),
      _n(
        'p-ahmed-samir',
        'Ahmed Samir',
        'person',
        -120,
        -100,
        cluster: 'k-clients',
      ),
      _n(
        'c-acme-logistics',
        'Acme Logistics',
        'company',
        20,
        170,
        cluster: 'k-clients',
      ),
    ],
    edges: [
      _e('n-pricing-experiments', 'k-pricing', 'concept'),
      _e('n-pricing-experiments', 'k-loyalty', 'relation:related'),
      _e('n-pricing-experiments', 'n-subscription-tiers', 'relation:part-of'),
      _e(
        'n-churn-notes',
        'n-pricing-experiments',
        'relation:supports',
        confidence: 0.77,
      ),
      _e(
        'n-pricing-experiments',
        'n-discount-policy',
        'relation:contradicts',
        confidence: 0.72,
      ),
      _e('n-pricing-experiments', 'n-call-acme', 'relation:follows-up'),
      _e('n-pricing-experiments', 'p-ahmed-samir', 'mention'),
      _e('n-pricing-experiments', 'c-acme-logistics', 'mention'),
    ],
  );

  /// A note that no longer exists.
  static const LocalGraphView missingLocal = LocalGraphView(
    center: 'n-gone',
    found: false,
    depth: 1,
    nodes: [],
    edges: [],
  );

  /// A small global map: three clusters, every node kind and edge style.
  static final GlobalGraphView globalSmall = GlobalGraphView(
    nodes: [
      _n(
        'n-pricing-experiments',
        'Pricing experiments',
        'note',
        0,
        0,
        degree: 9,
        cluster: 'k-pricing',
      ),
      _n(
        'k-pricing',
        'Pricing',
        'concept',
        -60,
        -70,
        degree: 6,
        cluster: 'k-pricing',
      ),
      _n(
        'n-subscription-tiers',
        'Subscription tiers',
        'note',
        80,
        -40,
        degree: 4,
        cluster: 'k-pricing',
      ),
      _n(
        'n-discount-policy',
        'Discount policy',
        'note',
        60,
        70,
        degree: 3,
        cluster: 'k-pricing',
      ),
      _n(
        'n-churn-notes',
        'Churn notes',
        'note',
        -80,
        60,
        degree: 5,
        cluster: 'k-pricing',
      ),
      _n(
        'n-pricing-experiment',
        'Pricing experiment',
        'note',
        20,
        120,
        degree: 1,
        cluster: 'k-pricing',
      ),
      _n(
        'c-acme-logistics',
        'Acme Logistics',
        'company',
        320,
        -20,
        degree: 7,
        cluster: 'k-clients',
      ),
      _n(
        'p-ahmed-samir',
        'Ahmed Samir',
        'person',
        260,
        -110,
        degree: 6,
        cluster: 'k-clients',
      ),
      _n(
        'n-call-acme',
        'Call 2026-09-12 — Acme',
        'note',
        230,
        40,
        degree: 5,
        cluster: 'k-clients',
      ),
      _n(
        'n-weekly-invoicing',
        'Weekly invoicing proposal',
        'note',
        400,
        70,
        degree: 3,
        cluster: 'k-clients',
      ),
      _n(
        'd-watanya-contract',
        'Watanya contract',
        'document',
        180,
        260,
        degree: 3,
        cluster: 'k-docs',
      ),
      _n(
        'pl-nasr-city-safe',
        'Safe — Nasr City office',
        'place',
        280,
        300,
        degree: 2,
        cluster: 'k-docs',
      ),
      _n(
        'pl-nasr-city-office',
        'Nasr City office',
        'place',
        380,
        330,
        degree: 1,
        cluster: 'k-docs',
      ),
      _n('p-shady', 'Shady', 'person', 120, 350, degree: 1, cluster: 'k-docs'),
      _n(
        'n-pricing-summary-ar',
        'تجارب التسعير — ملخص',
        'note',
        -170,
        -20,
        degree: 2,
      ),
    ],
    edges: [
      _e('n-pricing-experiments', 'k-pricing', 'concept'),
      _e('n-pricing-experiments', 'n-subscription-tiers', 'relation:part-of'),
      _e(
        'n-pricing-experiments',
        'n-discount-policy',
        'relation:contradicts',
        confidence: 0.72,
      ),
      _e('n-churn-notes', 'n-pricing-experiments', 'relation:supports'),
      _e(
        'n-pricing-experiment',
        'n-pricing-experiments',
        'relation:duplicates',
        confidence: 0.91,
      ),
      _e('n-pricing-summary-ar', 'n-pricing-experiments', 'link'),
      _e('n-churn-notes', 'n-subscription-tiers', 'similarity'),
      _e('n-call-acme', 'n-pricing-experiments', 'relation:follows-up'),
      _e('n-call-acme', 'p-ahmed-samir', 'mention'),
      _e('n-call-acme', 'c-acme-logistics', 'mention'),
      _e('p-ahmed-samir', 'c-acme-logistics', 'entity:works-at'),
      _e('n-weekly-invoicing', 'c-acme-logistics', 'relation:related'),
      _e('d-watanya-contract', 'pl-nasr-city-safe', 'custody:location'),
      _e('d-watanya-contract', 'p-shady', 'custody:last-holder'),
      _e('pl-nasr-city-safe', 'pl-nasr-city-office', 'part-of-place'),
    ],
    clusters: const [
      ClusterLabel(id: 'k-clients', name: 'Clients · Acme', size: 4),
      ClusterLabel(id: 'k-docs', name: 'Documents & places', size: 4),
      ClusterLabel(id: 'k-pricing', name: 'Pricing & plans', size: 6),
    ],
  );

  /// The neighbourhood of Acme Logistics (selection highlight).
  static final LocalGraphView acmeLocal = LocalGraphView(
    center: 'c-acme-logistics',
    found: true,
    depth: 1,
    nodes: [
      _n('c-acme-logistics', 'Acme Logistics', 'company', 0, 0, depth: 0),
      _n('p-ahmed-samir', 'Ahmed Samir', 'person', -60, -90),
      _n('n-call-acme', 'Call 2026-09-12 — Acme', 'note', -90, 60),
      _n('n-weekly-invoicing', 'Weekly invoicing proposal', 'note', 80, 70),
    ],
    edges: [
      _e('n-call-acme', 'c-acme-logistics', 'mention'),
      _e('p-ahmed-samir', 'c-acme-logistics', 'entity:works-at'),
      _e('n-weekly-invoicing', 'c-acme-logistics', 'relation:related'),
    ],
  );
}
