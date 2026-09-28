import 'package:strata_ui/strata_ui.dart';

/// How an edge of the core's graph views is drawn and filtered. One value per
/// `graph-algo` edge kind string (1:1 rendering map, no parsing):
/// `link`/`embed`, `relation:<type>`, `similarity`, `mention`, `concept`,
/// `custody:<field>`, `part-of-place`, and entity relations (`entity:<type>`,
/// the fallback).
enum EdgeClass {
  /// `relation:related`.
  related,

  /// `relation:part-of`.
  partOf,

  /// `relation:supports`.
  supports,

  /// `relation:contradicts`.
  contradicts,

  /// `relation:follows-up`.
  followsUp,

  /// `relation:duplicates`.
  duplicates,

  /// `similarity` (AI, ephemeral).
  similarity,

  /// `link` / `embed`.
  bodyLink,

  /// `mention` (note → person/company).
  mention,

  /// `concept` (note → concept).
  concept,

  /// `custody:location` / `custody:holder` / `custody:last-holder`.
  custody,

  /// `part-of-place` (place → place).
  placeNesting,

  /// `entity:<type>` and any kind this app version does not know.
  entity,
}

/// The edge class of a core edge kind string.
EdgeClass edgeClassOf(String kind) => switch (kind) {
  'relation:related' => EdgeClass.related,
  'relation:part-of' => EdgeClass.partOf,
  'relation:supports' => EdgeClass.supports,
  'relation:contradicts' => EdgeClass.contradicts,
  'relation:follows-up' => EdgeClass.followsUp,
  'relation:duplicates' => EdgeClass.duplicates,
  'similarity' => EdgeClass.similarity,
  'link' || 'embed' => EdgeClass.bodyLink,
  'mention' => EdgeClass.mention,
  'concept' => EdgeClass.concept,
  'custody:location' ||
  'custody:holder' ||
  'custody:last-holder' => EdgeClass.custody,
  'part-of-place' => EdgeClass.placeNesting,
  _ => EdgeClass.entity,
};

/// The core edge kinds drawn as [edgeClass] (the inverse of [edgeClassOf]),
/// for the edge-type selection the core filters by.
List<String> coreEdgeKindsOf(EdgeClass edgeClass) => switch (edgeClass) {
  EdgeClass.related => const ['relation:related'],
  EdgeClass.partOf => const ['relation:part-of'],
  EdgeClass.supports => const ['relation:supports'],
  EdgeClass.contradicts => const ['relation:contradicts'],
  EdgeClass.followsUp => const ['relation:follows-up'],
  EdgeClass.duplicates => const ['relation:duplicates'],
  EdgeClass.similarity => const ['similarity'],
  EdgeClass.bodyLink => const ['link', 'embed'],
  EdgeClass.mention => const ['mention'],
  EdgeClass.concept => const ['concept'],
  EdgeClass.custody => const ['custody'],
  EdgeClass.placeNesting => const ['part-of-place'],
  EdgeClass.entity => const ['entity', 'document', 'tag', 'co-mention'],
};

/// The core edge-kind selection for the classes the user left on: empty
/// (every kind) when nothing is switched off.
List<String> selectedEdgeKinds(Set<EdgeClass> hidden) => hidden.isEmpty
    ? const []
    : [
        for (final edgeClass in EdgeClass.values)
          if (!hidden.contains(edgeClass)) ...coreEdgeKindsOf(edgeClass),
      ];

/// The design-system relation type an edge class is drawn as.
RelationType relationTypeOf(EdgeClass edgeClass) => switch (edgeClass) {
  EdgeClass.related ||
  EdgeClass.concept ||
  EdgeClass.entity ||
  EdgeClass.custody => RelationType.related,
  EdgeClass.partOf => RelationType.partOf,
  EdgeClass.supports => RelationType.supports,
  EdgeClass.contradicts => RelationType.contradicts,
  EdgeClass.followsUp || EdgeClass.placeNesting => RelationType.followsUp,
  EdgeClass.duplicates => RelationType.duplicates,
  EdgeClass.similarity => RelationType.similarity,
  EdgeClass.bodyLink => RelationType.bodyLink,
  EdgeClass.mention => RelationType.mention,
};

/// The line style of an edge class (GraphLanguage: custody is solid 1.5 with
/// a square end, place nesting dotted 1-3).
RelationLineStyle lineStyleOf(EdgeClass edgeClass) => switch (edgeClass) {
  EdgeClass.custody => const RelationLineStyle(strokeWidth: 1.5),
  EdgeClass.placeNesting => const RelationLineStyle(
    strokeWidth: 1.5,
    dashPattern: [1, 3],
  ),
  _ => RelationLineStyle.of(relationTypeOf(edgeClass)),
};

/// The typed-relation edge classes the user can retype or reject, with the
/// vault's relation type string (`part-of`, …) — the six AI relation types of
/// PLAN §9.4.
const Map<EdgeClass, String> relationWireTypes = {
  EdgeClass.related: 'related',
  EdgeClass.partOf: 'part-of',
  EdgeClass.supports: 'supports',
  EdgeClass.contradicts: 'contradicts',
  EdgeClass.followsUp: 'follows-up',
  EdgeClass.duplicates: 'duplicates',
};

/// The design-system node kind of a core node kind string (`note`,
/// `concept`, `person`, `company`, `document`, `place`; anything else is
/// drawn as a note).
NodeKind nodeKindOf(String kind) => switch (kind) {
  'concept' => NodeKind.concept,
  'person' => NodeKind.person,
  'company' => NodeKind.company,
  'document' => NodeKind.document,
  'place' => NodeKind.place,
  _ => NodeKind.note,
};

/// Edge classes offered as filters on the global map, in panel order.
const List<EdgeClass> filterableEdgeClasses = [
  EdgeClass.related,
  EdgeClass.partOf,
  EdgeClass.supports,
  EdgeClass.contradicts,
  EdgeClass.followsUp,
  EdgeClass.duplicates,
  EdgeClass.bodyLink,
  EdgeClass.mention,
];

/// Node kinds offered as filters on the global map, in panel order.
const List<NodeKind> filterableNodeKinds = [
  NodeKind.note,
  NodeKind.concept,
  NodeKind.person,
  NodeKind.company,
  NodeKind.document,
  NodeKind.place,
];
