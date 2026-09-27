import 'package:flutter/material.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Rendering maps from the core's string enums to design-system variants
/// (1:1, no logic).
abstract final class CoreLabels {
  /// The design-system relation type of a core `rel_type` string.
  static RelationType relationType(String relType) => switch (relType) {
    'part-of' => RelationType.partOf,
    'supports' => RelationType.supports,
    'contradicts' => RelationType.contradicts,
    'follows-up' => RelationType.followsUp,
    'duplicates' => RelationType.duplicates,
    'similar' || 'similarity' => RelationType.similarity,
    'mention' || 'mentions' => RelationType.mention,
    'link' => RelationType.bodyLink,
    _ => RelationType.related,
  };

  /// The node kind of a core item `kind` string, when it has a glyph.
  static NodeKind? nodeKind(String kind) => switch (kind) {
    'note' => NodeKind.note,
    'concept' => NodeKind.concept,
    'person' => NodeKind.person,
    'company' => NodeKind.company,
    'document' => NodeKind.document,
    'place' => NodeKind.place,
    _ => null,
  };

  /// The localized name of an item `kind` (`task`, `note`, `person`, …).
  static String itemKind(TasksLocalizations l10n, String kind) =>
      switch (kind) {
        'task' => l10n.kindTask,
        'note' => l10n.kindNote,
        'person' => l10n.kindPerson,
        'company' => l10n.kindCompany,
        'concept' => l10n.kindConcept,
        'document' => l10n.kindDocument,
        'place' => l10n.kindPlace,
        _ => l10n.kindItem,
      };

  /// The localized duplicate match level (`exact` | `near` | `semantic`).
  static String matchLevel(TasksLocalizations l10n, String level) =>
      switch (level) {
        'exact' => l10n.dupMatchExact,
        'near' => l10n.dupMatchNear,
        'semantic' => l10n.dupMatchSemantic,
        _ => level,
      };

  /// A confidence or score as shown next to "AI" (two decimals, as on the
  /// relation chips of `strata_ui`).
  static String score(double value) => value.toStringAsFixed(2);
}

/// Forwards an intent to the core and reports a failure in a snack bar
/// (the core already applied or refused it; nothing is retried here).
Future<void> forwardIntent(BuildContext context, Future<Object?> intent) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final l10n =
      Localizations.of<TasksLocalizations>(context, TasksLocalizations) ??
      lookupTasksLocalizations(Localizations.localeOf(context));
  try {
    await intent;
  } on Object catch (error) {
    messenger?.showSnackBar(
      SnackBar(
        content: Text(
          error is CoreFailure
              ? l10n.commonIntentFailedCode(code: error.code)
              : l10n.commonIntentFailed,
        ),
      ),
    );
  }
}

/// A linked note or entity (`EntityRef`) as a compact chip: its title, a
/// node glyph when the kind is known, and a link action when the reference
/// resolves locally (`id` set).
class EntityRefChip extends StatelessWidget {
  /// Creates the chip.
  const new({
    required this.entity,
    super.key,
    this.kind,
    this.onOpen,
  });

  /// The reference.
  final EntityRef entity;

  /// The entity kind, when the view-model says it.
  final NodeKind? kind;

  /// Opens the entity by id.
  final ValueChanged<String>? onOpen;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final id = entity.id;
    final open = onOpen;
    final k = kind;
    final chip = Container(
      constraints: const BoxConstraints(minHeight: 28),
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s3,
        vertical: 2,
      ),
      decoration: BoxDecoration(
        color: colors.surface2,
        borderRadius: StrataRadii.pillRadius,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (k != null)
            NodeKindGlyph(kind: k, size: 12, decorative: true)
          else
            Icon(Icons.link, size: 14, color: colors.text2),
          const SizedBox(width: StrataSpacing.s1 + 2),
          Flexible(
            child: Text(
              entity.title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: text.bodySmall.withWeight(FontWeight.w500),
            ),
          ),
        ],
      ),
    );
    return StrataTapTarget(
      semanticLabel: entity.title,
      onTap: id == null || open == null ? null : () => open(id),
      child: chip,
    );
  }
}
