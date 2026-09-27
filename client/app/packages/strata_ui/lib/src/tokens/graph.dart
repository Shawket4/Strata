import 'package:flutter/material.dart';

/// Typed relation kinds (SCREEN_SPEC "Relation types"). Meaning is never
/// carried by colour alone: every type also has a distinct [RelationLineStyle].
enum RelationType {
  /// Generic relation: solid 1.5.
  related,

  /// Part-of: solid 2 with arrow.
  partOf,

  /// Supports: solid 2 with arrow.
  supports,

  /// Contradicts: dashed 6-4 with arrow and a small cross at the midpoint.
  contradicts,

  /// Follows-up: dotted 2-3 with arrow.
  followsUp,

  /// Duplicates: double parallel lines.
  duplicates,

  /// AI similarity (ephemeral): thin dotted 1-4 at 40 % opacity.
  similarity,

  /// Plain body link: hairline.
  bodyLink,

  /// Mention of a person/company: 1 px in the entity kind's colour.
  mention,
}

/// How a relation is drawn as a chip glyph or graph edge.
@immutable
class RelationLineStyle {
  /// Creates a line style.
  const new({
    required this.strokeWidth,
    this.dashPattern,
    this.arrow = false,
    this.doubleLine = false,
    this.midCross = false,
    this.opacity = 1,
  });

  /// Stroke width in logical pixels.
  final double strokeWidth;

  /// Alternating on/off lengths; `null` for a solid line.
  final List<double>? dashPattern;

  /// Whether the edge ends in an arrow head.
  final bool arrow;

  /// Whether the edge is drawn as two parallel lines.
  final bool doubleLine;

  /// Whether a small cross is drawn at the midpoint.
  final bool midCross;

  /// Opacity applied to the colour.
  final double opacity;

  /// The line style of [type], from the design spec.
  static RelationLineStyle of(RelationType type) => switch (type) {
    RelationType.related => const RelationLineStyle(strokeWidth: 1.5),
    RelationType.partOf => const RelationLineStyle(strokeWidth: 2, arrow: true),
    RelationType.supports => const RelationLineStyle(
      strokeWidth: 2,
      arrow: true,
    ),
    RelationType.contradicts => const RelationLineStyle(
      strokeWidth: 2,
      dashPattern: [6, 4],
      arrow: true,
      midCross: true,
    ),
    RelationType.followsUp => const RelationLineStyle(
      strokeWidth: 2,
      dashPattern: [2, 3],
      arrow: true,
    ),
    RelationType.duplicates => const RelationLineStyle(
      strokeWidth: 1.5,
      doubleLine: true,
    ),
    RelationType.similarity => const RelationLineStyle(
      strokeWidth: 1,
      dashPattern: [1, 4],
      opacity: 0.4,
    ),
    RelationType.bodyLink => const RelationLineStyle(strokeWidth: 1),
    RelationType.mention => const RelationLineStyle(strokeWidth: 1),
  };
}

/// Graph node kinds (SCREEN_SPEC "Node kinds" and "Additions").
enum NodeKind {
  /// A note: filled circle.
  note,

  /// A concept: diamond with tide stroke.
  concept,

  /// A person: circle with an outer ring.
  person,

  /// A company: rounded square.
  company,

  /// A document: page with a folded corner.
  document,

  /// A place: map-pin outline.
  place,
}

/// The shape a [NodeKind] is drawn with.
enum NodeShape {
  /// Filled circle.
  circle,

  /// Rotated square.
  diamond,

  /// Circle with an outer ring at a 2 px gap.
  ringedCircle,

  /// Rounded square.
  roundedSquare,

  /// Page with a folded top corner.
  foldedPage,

  /// Map-pin outline.
  pin,
}

/// Shape per node kind.
NodeShape nodeShapeOf(NodeKind kind) => switch (kind) {
  NodeKind.note => NodeShape.circle,
  NodeKind.concept => NodeShape.diamond,
  NodeKind.person => NodeShape.ringedCircle,
  NodeKind.company => NodeShape.roundedSquare,
  NodeKind.document => NodeShape.foldedPage,
  NodeKind.place => NodeShape.pin,
};

/// Fill and stroke of one node kind.
@immutable
class NodeKindColors {
  /// Creates node colours; a `null` [fill] means an outline-only shape.
  const new({required this.stroke, this.fill});

  /// Fill colour, or `null` for outline shapes (place).
  final Color? fill;

  /// Stroke colour (equals the fill for solid shapes).
  final Color stroke;

  /// Linear interpolation used by theme animation.
  static NodeKindColors lerp(NodeKindColors a, NodeKindColors b, double t) =>
      NodeKindColors(
        fill: Color.lerp(a.fill, b.fill, t),
        stroke: Color.lerp(a.stroke, b.stroke, t)!,
      );
}

/// Relation and node colours for one brightness, as a [ThemeExtension].
@immutable
class StrataGraphColors extends ThemeExtension<StrataGraphColors> {
  /// Creates a graph colour set.
  const new({
    required this.related,
    required this.partOf,
    required this.supports,
    required this.contradicts,
    required this.followsUp,
    required this.duplicates,
    required this.similarity,
    required this.bodyLink,
    required this.note,
    required this.concept,
    required this.person,
    required this.company,
    required this.document,
    required this.place,
    required this.clusterFill,
    required this.clusterOutline,
    required this.selectedRing,
  });

  /// Light graph colours from the spec.
  static const StrataGraphColors light = StrataGraphColors(
    related: Color(0xFF52616B),
    partOf: Color(0xFF0F1B26),
    supports: Color(0xFF2F7A55),
    contradicts: Color(0xFFB3412E),
    followsUp: Color(0xFF1D5C8C),
    duplicates: Color(0xFF7C8C96),
    similarity: Color(0xFF7C8C96),
    bodyLink: Color(0xFF9AAAB3),
    note: NodeKindColors(fill: Color(0xFF0F1B26), stroke: Color(0xFF0F1B26)),
    concept: NodeKindColors(
      fill: Color(0xFFDCEAF4),
      stroke: Color(0xFF2477B3),
    ),
    person: NodeKindColors(fill: Color(0xFF2F7A55), stroke: Color(0xFF2F7A55)),
    company: NodeKindColors(
      fill: Color(0xFF9A6A12),
      stroke: Color(0xFF9A6A12),
    ),
    document: NodeKindColors(
      fill: Color(0xFF3F4C55),
      stroke: Color(0xFF3F4C55),
    ),
    place: NodeKindColors(stroke: Color(0xFF1D5C8C)),
    clusterFill: Color(0x122477B3),
    clusterOutline: Color(0xFF7C8C96),
    selectedRing: Color(0xFF2477B3),
  );

  /// Dark graph colours: spec values where given (note, person, company,
  /// document, place), otherwise derived for contrast on the dark ground.
  static const StrataGraphColors dark = StrataGraphColors(
    related: Color(0xFF93A3AD),
    partOf: Color(0xFFF1F5F7),
    supports: Color(0xFF6CC495),
    contradicts: Color(0xFFE07A66),
    followsUp: Color(0xFF6CB4DD),
    duplicates: Color(0xFF7C8C96),
    similarity: Color(0xFF7C8C96),
    bodyLink: Color(0xFF5E7686),
    note: NodeKindColors(fill: Color(0xFFF1F5F7), stroke: Color(0xFFF1F5F7)),
    concept: NodeKindColors(
      fill: Color(0xFF1B3A55),
      stroke: Color(0xFF6CB4DD),
    ),
    person: NodeKindColors(fill: Color(0xFF6CC495), stroke: Color(0xFF6CC495)),
    company: NodeKindColors(
      fill: Color(0xFFC99A3E),
      stroke: Color(0xFFC99A3E),
    ),
    document: NodeKindColors(
      fill: Color(0xFFB9C8D0),
      stroke: Color(0xFFB9C8D0),
    ),
    place: NodeKindColors(stroke: Color(0xFF6CB4DD)),
    clusterFill: Color(0x122477B3),
    clusterOutline: Color(0xFF7C8C96),
    selectedRing: Color(0xFF2477B3),
  );

  /// Colour of [RelationType.related].
  final Color related;

  /// Colour of [RelationType.partOf].
  final Color partOf;

  /// Colour of [RelationType.supports].
  final Color supports;

  /// Colour of [RelationType.contradicts].
  final Color contradicts;

  /// Colour of [RelationType.followsUp].
  final Color followsUp;

  /// Colour of [RelationType.duplicates].
  final Color duplicates;

  /// Base colour of [RelationType.similarity] (drawn at 40 % opacity).
  final Color similarity;

  /// Colour of [RelationType.bodyLink].
  final Color bodyLink;

  /// Note node colours.
  final NodeKindColors note;

  /// Concept node colours.
  final NodeKindColors concept;

  /// Person node colours (also person mentions).
  final NodeKindColors person;

  /// Company node colours (also company mentions).
  final NodeKindColors company;

  /// Document node colours.
  final NodeKindColors document;

  /// Place node colours.
  final NodeKindColors place;

  /// Cluster region fill (tide at 7 %).
  final Color clusterFill;

  /// Cluster region dashed outline.
  final Color clusterOutline;

  /// 3 px ring around a selected node.
  final Color selectedRing;

  /// Colour of a relation. A [RelationType.mention] takes the colour of the
  /// mentioned entity's [mentionOf] kind (person by default).
  Color relation(RelationType type, {NodeKind mentionOf = NodeKind.person}) =>
      switch (type) {
        RelationType.related => related,
        RelationType.partOf => partOf,
        RelationType.supports => supports,
        RelationType.contradicts => contradicts,
        RelationType.followsUp => followsUp,
        RelationType.duplicates => duplicates,
        RelationType.similarity => similarity,
        RelationType.bodyLink => bodyLink,
        RelationType.mention => node(mentionOf).stroke,
      };

  /// Colours of a node kind.
  NodeKindColors node(NodeKind kind) => switch (kind) {
    NodeKind.note => note,
    NodeKind.concept => concept,
    NodeKind.person => person,
    NodeKind.company => company,
    NodeKind.document => document,
    NodeKind.place => place,
  };

  @override
  StrataGraphColors copyWith({
    Color? related,
    Color? partOf,
    Color? supports,
    Color? contradicts,
    Color? followsUp,
    Color? duplicates,
    Color? similarity,
    Color? bodyLink,
    NodeKindColors? note,
    NodeKindColors? concept,
    NodeKindColors? person,
    NodeKindColors? company,
    NodeKindColors? document,
    NodeKindColors? place,
    Color? clusterFill,
    Color? clusterOutline,
    Color? selectedRing,
  }) {
    return StrataGraphColors(
      related: related ?? this.related,
      partOf: partOf ?? this.partOf,
      supports: supports ?? this.supports,
      contradicts: contradicts ?? this.contradicts,
      followsUp: followsUp ?? this.followsUp,
      duplicates: duplicates ?? this.duplicates,
      similarity: similarity ?? this.similarity,
      bodyLink: bodyLink ?? this.bodyLink,
      note: note ?? this.note,
      concept: concept ?? this.concept,
      person: person ?? this.person,
      company: company ?? this.company,
      document: document ?? this.document,
      place: place ?? this.place,
      clusterFill: clusterFill ?? this.clusterFill,
      clusterOutline: clusterOutline ?? this.clusterOutline,
      selectedRing: selectedRing ?? this.selectedRing,
    );
  }

  @override
  StrataGraphColors lerp(StrataGraphColors? other, double t) {
    if (other == null) return this;
    Color l(Color a, Color b) => Color.lerp(a, b, t)!;
    NodeKindColors n(NodeKindColors a, NodeKindColors b) =>
        NodeKindColors.lerp(a, b, t);
    return StrataGraphColors(
      related: l(related, other.related),
      partOf: l(partOf, other.partOf),
      supports: l(supports, other.supports),
      contradicts: l(contradicts, other.contradicts),
      followsUp: l(followsUp, other.followsUp),
      duplicates: l(duplicates, other.duplicates),
      similarity: l(similarity, other.similarity),
      bodyLink: l(bodyLink, other.bodyLink),
      note: n(note, other.note),
      concept: n(concept, other.concept),
      person: n(person, other.person),
      company: n(company, other.company),
      document: n(document, other.document),
      place: n(place, other.place),
      clusterFill: l(clusterFill, other.clusterFill),
      clusterOutline: l(clusterOutline, other.clusterOutline),
      selectedRing: l(selectedRing, other.selectedRing),
    );
  }
}
