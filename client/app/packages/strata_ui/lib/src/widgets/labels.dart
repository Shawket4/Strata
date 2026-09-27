import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/tokens/graph.dart';

/// Localized display names of the design-system enums.
extension StrataEnumLabels on StrataLocalizations {
  /// The name of a relation type as shown on chips and legends.
  String relationTypeLabel(RelationType type) => switch (type) {
    RelationType.related => relationRelated,
    RelationType.partOf => relationPartOf,
    RelationType.supports => relationSupports,
    RelationType.contradicts => relationContradicts,
    RelationType.followsUp => relationFollowsUp,
    RelationType.duplicates => relationDuplicates,
    RelationType.similarity => relationSimilarity,
    RelationType.bodyLink => relationBodyLink,
    RelationType.mention => relationMention,
  };

  /// The name of a node kind.
  String nodeKindLabel(NodeKind kind) => switch (kind) {
    NodeKind.note => nodeKindNote,
    NodeKind.concept => nodeKindConcept,
    NodeKind.person => nodeKindPerson,
    NodeKind.company => nodeKindCompany,
    NodeKind.document => nodeKindDocument,
    NodeKind.place => nodeKindPlace,
  };
}
