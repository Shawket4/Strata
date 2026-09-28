import 'package:flutter/widgets.dart';
import 'package:strata_editor/strata_editor.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_ui/strata_ui.dart';

/// Makes the notes and editor strings available below it (the app only
/// registers the shared `StrataLocalizations`).
class NotesLocalizationsScope extends StatelessWidget {
  /// Creates the scope around [child].
  const new({required this.child, super.key});

  /// The subtree.
  final Widget child;

  @override
  Widget build(BuildContext context) => Localizations.override(
    context: context,
    delegates: const [
      NotesLocalizations.delegate,
      EditorLocalizations.delegate,
    ],
    child: child,
  );
}

/// Relation keys the core writes (PLAN §6.4), mapped 1:1 to the design
/// system's relation types; the entity keys are mentions.
RelationType relationTypeOf(String relType) => switch (relType) {
  'part-of' => RelationType.partOf,
  'supports' => RelationType.supports,
  'contradicts' => RelationType.contradicts,
  'follows-up' => RelationType.followsUp,
  'duplicates' => RelationType.duplicates,
  'people' || 'companies' || 'concepts' => RelationType.mention,
  'link' || 'embed' => RelationType.bodyLink,
  _ => RelationType.related,
};

/// The entity kind of a mention relation key (its colour).
NodeKind mentionKindOf(String relType) => switch (relType) {
  'companies' => NodeKind.company,
  'concepts' => NodeKind.concept,
  'people' => NodeKind.person,
  _ => NodeKind.note,
};
