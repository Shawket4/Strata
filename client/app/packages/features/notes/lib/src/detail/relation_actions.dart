import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_notes/src/notes_scope.dart';
import 'package:strata_state/strata_state.dart' as core;
import 'package:strata_state/strata_state.dart' show coreApiProvider;
import 'package:strata_ui/strata_ui.dart';

/// The typed relation chip of a note's outgoing relation: tap opens the
/// target; an AI relation shows `AI · 0.72` and offers Reject / Retype in a
/// hover card (pointer, medium and expanded) or a long-press sheet (touch).
class NoteRelationChip extends StatefulWidget {
  /// Creates the chip for [relation] of note [noteId].
  const new({
    required this.noteId,
    required this.relation,
    super.key,
    this.onOpenNote,
    this.hoverCard = false,
  });

  /// The note the relation is stored on.
  final String noteId;

  /// The relation.
  final core.RelationChip relation;

  /// Opens a note.
  final ValueChanged<String>? onOpenNote;

  /// Whether hovering shows the reason card (pointer layouts).
  final bool hoverCard;

  @override
  State<NoteRelationChip> createState() => _NoteRelationChipState();
}

class _NoteRelationChipState extends State<NoteRelationChip> {
  final OverlayPortalController _portal = OverlayPortalController();
  final LayerLink _link = LayerLink();
  bool _chipHovered = false;
  bool _cardHovered = false;

  bool get _isAi => widget.relation.by == 'ai';

  void _hover({bool? chip, bool? card}) {
    _chipHovered = chip ?? _chipHovered;
    _cardHovered = card ?? _cardHovered;
    if (_chipHovered || _cardHovered) {
      _portal.show();
    } else {
      _portal.hide();
    }
  }

  @override
  Widget build(BuildContext context) {
    final relation = widget.relation;
    final target = relation.target.id;
    final open = widget.onOpenNote;
    final chip = RelationChip(
      type: relationTypeOf(relation.relType),
      mentionOf: mentionKindOf(relation.relType),
      label: relation.target.title,
      aiConfidence: _isAi ? relation.confidence : null,
      onPressed: target == null || open == null ? null : () => open(target),
      onLongPress: _isAi
          ? () => unawaited(
              showAiRelationSheet(
                context,
                noteId: widget.noteId,
                relation: relation,
              ),
            )
          : null,
    );
    if (!_isAi || !widget.hoverCard) return chip;
    return CompositedTransformTarget(
      link: _link,
      child: OverlayPortal(
        controller: _portal,
        overlayChildBuilder: (overlayContext) => CompositedTransformFollower(
          link: _link,
          targetAnchor: AlignmentDirectional.bottomStart.resolve(
            Directionality.of(context),
          ),
          followerAnchor: AlignmentDirectional.topStart.resolve(
            Directionality.of(context),
          ),
          child: Align(
            alignment: AlignmentDirectional.topStart,
            child: MouseRegion(
              onEnter: (_) => _hover(card: true),
              onExit: (_) => _hover(card: false),
              child: SizedBox(
                width: 340,
                child: Material(
                  color: context.strataColors.surface,
                  shape: RoundedRectangleBorder(
                    borderRadius: StrataRadii.cardRadius,
                    side: BorderSide(color: context.strataColors.border),
                  ),
                  shadowColor: Colors.transparent,
                  child: DecoratedBox(
                    decoration: const BoxDecoration(
                      borderRadius: StrataRadii.cardRadius,
                      boxShadow: StrataElevation.popover,
                    ),
                    child: AiRelationCard(
                      noteId: widget.noteId,
                      relation: relation,
                      onDone: () => _hover(chip: false, card: false),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
        child: MouseRegion(
          onEnter: (_) => _hover(chip: true),
          onExit: (_) => _hover(chip: false),
          child: chip,
        ),
      ),
    );
  }
}

/// Shows the AI relation's reason with Reject / Retype in a bottom sheet
/// (phones and touch).
Future<void> showAiRelationSheet(
  BuildContext context, {
  required String noteId,
  required core.RelationChip relation,
}) => showModalBottomSheet<void>(
  context: context,
  useSafeArea: true,
  isScrollControlled: true,
  builder: (sheetContext) => NotesLocalizationsScope(
    child: SafeArea(
      child: AiRelationCard(
        noteId: noteId,
        relation: relation,
        onDone: () => Navigator.of(sheetContext).pop(),
      ),
    ),
  ),
);

/// "Suggested by AI · confidence 0.72", the AI's reason, and Reject /
/// Retype (`removeRelation` / `retypeRelation`).
class AiRelationCard extends ConsumerWidget {
  /// Creates the card.
  const new({
    required this.noteId,
    required this.relation,
    super.key,
    this.onDone,
  });

  /// The note the relation is stored on.
  final String noteId;

  /// The relation.
  final core.RelationChip relation;

  /// Called after an action (closes the sheet / card).
  final VoidCallback? onDone;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final confidence = relation.confidence;
    final target = relation.target.id;
    final reason = relation.reason;
    final type = relationTypeOf(relation.relType);
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        StrataSpacing.s4,
        StrataSpacing.s3,
        StrataSpacing.s4,
        StrataSpacing.s3,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: StrataSpacing.s2,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              RelationLineSample(type: type),
              Text(
                l10n.aiSuggestedBy,
                style: text.bodySmall.copyWith(
                  color: colors.text,
                  fontWeight: FontWeight.w600,
                ),
              ),
              if (confidence != null)
                Text(
                  l10n.aiConfidence(value: confidence.toStringAsFixed(2)),
                  style: text.caption.copyWith(color: colors.text2),
                ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s1),
          Text(
            l10n.relationLine(
              type: isNamedRelation(relation.relType)
                  ? context.l10n.relationTypeLabel(type)
                  : relation.relType,
              title: relation.target.title,
            ),
            style: text.caption.copyWith(color: colors.text2),
          ),
          const SizedBox(height: StrataSpacing.s2),
          Text(
            reason ?? l10n.noReason,
            style: text.bodySmall.copyWith(color: colors.text),
          ),
          const SizedBox(height: StrataSpacing.s3),
          Wrap(
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s2,
            children: [
              OutlinedButton.icon(
                onPressed: target == null
                    ? null
                    : () {
                        unawaited(
                          ref
                              .read(coreApiProvider)
                              .removeRelation(
                                srcId: noteId,
                                dstId: target,
                                relType: relation.relType,
                              ),
                        );
                        onDone?.call();
                      },
                style: OutlinedButton.styleFrom(
                  foregroundColor: colors.dangerText,
                  side: BorderSide(color: colors.danger),
                  minimumSize: const Size(64, StrataLayout.minTouchTarget),
                ),
                icon: const Icon(Icons.close, size: 18),
                label: Text(l10n.relationReject),
              ),
              OutlinedButton.icon(
                onPressed: target == null
                    ? null
                    : () => unawaited(_retype(context, ref, target)),
                style: OutlinedButton.styleFrom(
                  foregroundColor: colors.text,
                  side: BorderSide(color: colors.border),
                  minimumSize: const Size(64, StrataLayout.minTouchTarget),
                ),
                icon: const Icon(Icons.swap_horiz, size: 18),
                label: Text(l10n.relationRetype),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _retype(
    BuildContext context,
    WidgetRef ref,
    String target,
  ) async {
    final api = ref.read(coreApiProvider);
    final chosen = await showDialog<String>(
      context: context,
      builder: (dialogContext) => NotesLocalizationsScope(
        child: Builder(
          builder: (context) => SimpleDialog(
            title: Text(NotesLocalizations.of(context).retypeTitle),
            children: [
              for (final entry in retypeTargets.entries)
                if (entry.key != relation.relType)
                  SimpleDialogOption(
                    onPressed: () => Navigator.of(dialogContext).pop(entry.key),
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(
                        minHeight: StrataLayout.minTouchTarget - 16,
                      ),
                      child: Row(
                        children: [
                          RelationLineSample(type: entry.value),
                          const SizedBox(width: StrataSpacing.s3),
                          Text(context.l10n.relationTypeLabel(entry.value)),
                        ],
                      ),
                    ),
                  ),
            ],
          ),
        ),
      ),
    );
    if (chosen == null) return;
    await api.retypeRelation(
      srcId: noteId,
      dstId: target,
      relType: relation.relType,
      newType: chosen,
    );
    onDone?.call();
  }
}
