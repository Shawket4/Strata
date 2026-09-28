import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_inbox/src/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// Where a suggestion is rendered.
enum ProposalDensity {
  /// A card in a one-column list (compact).
  card,

  /// The detail pane (medium / expanded): per-item controls are shown.
  detail,
}

/// The "AI · 0.88" tag of a confidence the core provides.
class AiConfidenceTag extends StatelessWidget {
  /// Creates the tag.
  const new({required this.confidence, super.key});

  /// The core's confidence.
  final double confidence;

  @override
  Widget build(BuildContext context) {
    return StatusPill(
      label: context.inboxL10n.inboxAiConfidence(
        score: CoreLabels.score(confidence),
      ),
      tone: StatusTone.info,
      icon: Icons.auto_awesome_outlined,
    );
  }
}

/// A small caps label above a proposal field.
class ProposalLabel extends StatelessWidget {
  /// Creates the label.
  const new(this.text, {super.key});

  /// The label.
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(
        top: StrataSpacing.s2,
        bottom: StrataSpacing.s1,
      ),
      child: Text(
        text,
        style: context.strataText.caption
            .withWeight(FontWeight.w700)
            .copyWith(color: context.strataColors.text2, letterSpacing: 0.4),
      ),
    );
  }
}

/// The content of a suggestion tied to a capture: filing (title, folder,
/// tags, people and companies), a typed relation (chip with AI confidence and
/// reason), a proposed task line, or — for kinds that need the user's
/// choice — the matching card with its own actions.
class ProposalContent extends ConsumerWidget {
  /// Creates the content of [suggestion].
  const new({
    required this.suggestion,
    super.key,
    this.density = ProposalDensity.card,
    this.onOpenNote,
    this.onOpenEntity,
  });

  /// The suggestion.
  final SuggestionItem suggestion;

  /// Card or detail rendering.
  final ProposalDensity density;

  /// Opens a note by id.
  final ValueChanged<String>? onOpenNote;

  /// Opens an entity by id.
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = suggestion.detail;
    final confidence = detail.confidence;
    switch (detail.kind) {
      case SuggestionKind.filing:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Wrap(
              spacing: StrataSpacing.s2,
              runSpacing: StrataSpacing.s1,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  detail.title,
                  style: text.body.withWeight(FontWeight.w600),
                ),
                if (confidence != null) AiConfidenceTag(confidence: confidence),
                if (suggestion.pendingSync) const NotSyncedMarker(),
              ],
            ),
            const SizedBox(height: 2),
            Wrap(
              spacing: StrataSpacing.s2,
              runSpacing: StrataSpacing.s1,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.folder_outlined, size: 16, color: colors.text2),
                    const SizedBox(width: StrataSpacing.s1),
                    Flexible(
                      child: Text(
                        detail.folder,
                        textDirection: TextDirection.ltr,
                        style: text.monoSmall.copyWith(color: colors.text2),
                      ),
                    ),
                  ],
                ),
                for (final tag in detail.tags)
                  Text(
                    l10n.inboxTag(tag: tag),
                    style: text.bodySmall.copyWith(color: colors.accentText),
                  ),
              ],
            ),
            if (detail.candidates.isNotEmpty) ...[
              ProposalLabel(l10n.inboxPeopleAndCompanies),
              Wrap(
                spacing: StrataSpacing.s2,
                runSpacing: StrataSpacing.s1,
                children: [
                  for (final entity in detail.candidates)
                    EntityRefChip(entity: entity, onOpen: onOpenEntity),
                ],
              ),
            ],
            if (density == ProposalDensity.detail) ...[
              const SizedBox(height: StrataSpacing.s2),
              Text(
                l10n.inboxCreatesNoteIn(folder: detail.folder),
                style: text.caption.copyWith(color: colors.text2),
              ),
            ],
          ],
        );
      case SuggestionKind.correction:
        final target = detail.target;
        final targetId = target?.id;
        final openNote = onOpenNote;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Flexible(
                  child: RelationChip(
                    type: CoreLabels.relationType(detail.relType),
                    label: target?.title ?? detail.relType,
                    aiConfidence: confidence,
                    onPressed: targetId == null || openNote == null
                        ? null
                        : () => openNote(targetId),
                  ),
                ),
                if (suggestion.pendingSync) ...[
                  const SizedBox(width: StrataSpacing.s1),
                  const NotSyncedMarker(),
                ],
                if (density == ProposalDensity.detail)
                  IconButton(
                    tooltip: l10n.inboxRejectRelation,
                    onPressed: () => unawaited(
                      forwardIntent(
                        context,
                        ref
                            .read(coreApiProvider)
                            .rejectSuggestion(id: suggestion.id),
                      ),
                    ),
                    icon: const Icon(Icons.close, size: 18),
                  ),
              ],
            ),
            if (detail.reason.isNotEmpty)
              Padding(
                padding: const EdgeInsetsDirectional.only(
                  start: StrataSpacing.s3,
                  top: 2,
                ),
                child: Text(
                  detail.reason,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              ),
          ],
        );
      case SuggestionKind.task:
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.task_alt, size: 18, color: colors.accentText),
            const SizedBox(width: StrataSpacing.s2),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.inboxTaskProposal,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                  Text(
                    detail.line,
                    style: text.bodySmall.withWeight(FontWeight.w500),
                  ),
                  if (confidence != null)
                    AiConfidenceTag(confidence: confidence),
                ],
              ),
            ),
          ],
        );
      case SuggestionKind.entityLink:
      case SuggestionKind.custody:
      case SuggestionKind.duplicate || SuggestionKind.duplicates:
      case SuggestionKind.unsupported || SuggestionKind.conflict:
        return SuggestionCard(
          suggestion: suggestion,
          onOpenNote: onOpenNote,
          onOpenEntity: onOpenEntity,
          nested: true,
        );
    }
  }
}

/// A suggestion that needs the user on its own: entity link-or-create,
/// custody (applied automatically with Undo, or a suggestion with a document
/// choice), duplicate-flagged capture (Open existing / Create anyway /
/// Discard), a relation or task proposal, or a kind this version cannot show.
/// Every action forwards to `CoreApi` (accept / reject suggestion, create
/// entity).
class SuggestionCard extends HookConsumerWidget {
  /// Creates the card.
  const new({
    required this.suggestion,
    super.key,
    this.onOpenNote,
    this.onOpenEntity,
    this.nested = false,
  });

  /// The suggestion.
  final SuggestionItem suggestion;

  /// Opens a note by id.
  final ValueChanged<String>? onOpenNote;

  /// Opens an entity by id.
  final ValueChanged<String>? onOpenEntity;

  /// Rendered inside a capture card (no outer card decoration).
  final bool nested;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = suggestion.detail;
    final api = ref.read(coreApiProvider);
    final pending = suggestion.status == 'pending';
    final applied = suggestion.status == 'accepted';
    final confidence = detail.confidence;

    void accept() => unawaited(
      forwardIntent(context, api.acceptSuggestion(id: suggestion.id)),
    );
    void reject() => unawaited(
      forwardIntent(context, api.rejectSuggestion(id: suggestion.id)),
    );

    final (
      String badge,
      StatusTone tone,
      IconData icon,
    ) = switch (detail.kind) {
      SuggestionKind.entityLink => (
        l10n.inboxNeedsYou,
        StatusTone.warning,
        Icons.person_search_outlined,
      ),
      SuggestionKind.custody when applied => (
        l10n.inboxAppliedAutomatically,
        StatusTone.success,
        Icons.check_circle_outline,
      ),
      SuggestionKind.custody => (
        l10n.inboxCustodySuggestion,
        StatusTone.warning,
        Icons.inventory_2_outlined,
      ),
      SuggestionKind.duplicate || SuggestionKind.duplicates => (
        l10n.inboxPossibleDuplicate,
        StatusTone.warning,
        Icons.content_copy_outlined,
      ),
      SuggestionKind.correction => (
        l10n.inboxRelationSuggestion,
        StatusTone.info,
        Icons.hub_outlined,
      ),
      SuggestionKind.task => (
        l10n.inboxTaskSuggestion,
        StatusTone.info,
        Icons.task_alt,
      ),
      SuggestionKind.filing => (
        l10n.inboxFilingProposal,
        StatusTone.info,
        Icons.drive_file_move_outline,
      ),
      SuggestionKind.unsupported || SuggestionKind.conflict => (
        l10n.inboxUnsupportedBadge,
        StatusTone.neutral,
        Icons.help_outline,
      ),
    };

    final Widget body;
    final List<Widget> actions;
    switch (detail.kind) {
      case SuggestionKind.entityLink:
        body = _LinkOrCreate(suggestion: suggestion);
        actions = [
          TextButton(
            onPressed: reject,
            child: Text(
              l10n.inboxReject,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ];
      case SuggestionKind.custody:
        body = _Custody(suggestion: suggestion, onOpenEntity: onOpenEntity);
        actions = applied
            ? [
                OutlinedButton.icon(
                  onPressed: reject,
                  icon: const Icon(Icons.undo),
                  label: Text(
                    l10n.inboxUndo,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ]
            : [
                TextButton(
                  onPressed: reject,
                  child: Text(
                    l10n.inboxReject,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (detail.candidates.isEmpty)
                  FilledButton(
                    style: tallFilledButton,
                    onPressed: accept,
                    child: Text(
                      l10n.inboxAccept,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  )
                else
                  Tooltip(
                    message: l10n.inboxChoiceUnavailable,
                    child: FilledButton(
                      style: tallFilledButton,
                      onPressed: null,
                      child: Text(
                        l10n.inboxAccept,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
              ];
      case SuggestionKind.duplicate || SuggestionKind.duplicates:
        body = DuplicateCandidatesView(
          kind: 'item',
          title: null,
          candidates: detail.duplicates,
          onOpenExisting: (candidate) => onOpenNote?.call(candidate.id),
          onCreateAnyway: reject,
          onCancel: accept,
          cancelLabel: l10n.inboxDiscard,
        );
        actions = const [];
      case SuggestionKind.correction:
      case SuggestionKind.task:
      case SuggestionKind.filing:
        body = ProposalContent(
          suggestion: suggestion,
          onOpenNote: onOpenNote,
          onOpenEntity: onOpenEntity,
        );
        actions = [
          TextButton(
            onPressed: reject,
            child: Text(
              l10n.inboxReject,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          FilledButton(
            style: tallFilledButton,
            onPressed: accept,
            child: Text(
              l10n.inboxAccept,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ];
      case SuggestionKind.unsupported || SuggestionKind.conflict:
        body = Text(
          l10n.inboxUnsupported(kind: detail.serverKind),
          style: text.bodySmall.copyWith(color: colors.text2),
        );
        actions = [
          TextButton(
            onPressed: reject,
            child: Text(
              l10n.inboxDismiss,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ];
    }

    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: StrataSpacing.s2,
          runSpacing: StrataSpacing.s1,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            StatusPill(label: badge, tone: tone, icon: icon),
            if (detail.kind == SuggestionKind.custody && confidence != null)
              AiConfidenceTag(confidence: confidence),
            if (suggestion.status == 'rejected')
              StatusPill(label: l10n.inboxRejected),
            if (suggestion.pendingSync) const NotSyncedMarker(),
          ],
        ),
        const SizedBox(height: StrataSpacing.s2),
        body,
        if (actions.isNotEmpty && (pending || applied)) ...[
          const SizedBox(height: StrataSpacing.s3),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s2,
            children: actions,
          ),
        ],
      ],
    );
    if (nested) {
      return DecoratedBox(
        decoration: BoxDecoration(
          color: colors.surface2,
          borderRadius: StrataRadii.cardRadius,
        ),
        child: Padding(
          padding: const EdgeInsets.all(StrataSpacing.s3),
          child: content,
        ),
      );
    }
    return Semantics(
      container: true,
      label: badge,
      child: Card(
        margin: EdgeInsets.zero,
        child: Padding(
          padding: const EdgeInsets.all(StrataSpacing.s4),
          child: content,
        ),
      ),
    );
  }
}

class _LinkOrCreate extends ConsumerWidget {
  const new({required this.suggestion});

  final SuggestionItem suggestion;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = suggestion.detail;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Semantics(
          header: true,
          container: true,
          child: Text(
            l10n.inboxWhoIs(mention: detail.mention),
            style: text.titleSmall,
          ),
        ),
        const SizedBox(height: 2),
        Text(
          detail.candidates.isEmpty
              ? l10n.inboxNoPersonMatches
              : l10n.inboxPossibleMatches,
          style: text.bodySmall.copyWith(color: colors.text2),
        ),
        if (detail.candidates.isNotEmpty) ...[
          const SizedBox(height: StrataSpacing.s2),
          Wrap(
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s1,
            children: [
              for (final candidate in detail.candidates)
                EntityRefChip(entity: candidate, kind: NodeKind.person),
            ],
          ),
        ],
        const SizedBox(height: StrataSpacing.s3),
        Wrap(
          spacing: StrataSpacing.s2,
          runSpacing: StrataSpacing.s2,
          children: [
            Tooltip(
              message: l10n.inboxLinkUnavailable,
              child: OutlinedButton.icon(
                onPressed: null,
                icon: const Icon(Icons.link),
                label: Text(
                  l10n.inboxLinkExisting,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
            FilledButton.tonalIcon(
              style: tallFilledButton,
              onPressed: () =>
                  CreatePersonSheet.show(context, suggestion: suggestion),
              icon: const Icon(Icons.person_add_alt),
              label: Text(
                l10n.inboxCreatePerson,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
        const SizedBox(height: StrataSpacing.s2),
        Text(
          l10n.inboxAliasNote(mention: detail.mention),
          style: text.caption.copyWith(color: colors.text2),
        ),
      ],
    );
  }
}

class _Custody extends StatelessWidget {
  const new({required this.suggestion, required this.onOpenEntity});

  final SuggestionItem suggestion;
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = suggestion.detail;
    final document = detail.document;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (document != null)
          EntityRefChip(
            entity: document,
            kind: NodeKind.document,
            onOpen: onOpenEntity,
          ),
        const SizedBox(height: StrataSpacing.s1),
        Text(detail.line, style: text.body.withWeight(FontWeight.w500)),
        if (detail.candidates.isNotEmpty) ...[
          const SizedBox(height: StrataSpacing.s3),
          Semantics(
            header: true,
            container: true,
            child: Text(l10n.inboxWhichDocument, style: text.titleSmall),
          ),
          const SizedBox(height: StrataSpacing.s1),
          for (final candidate in detail.candidates)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  const NodeKindGlyph(
                    kind: NodeKind.document,
                    size: 14,
                    decorative: true,
                  ),
                  const SizedBox(width: StrataSpacing.s2),
                  Expanded(child: Text(candidate.title, style: text.bodySmall)),
                ],
              ),
            ),
          const SizedBox(height: StrataSpacing.s1),
          Text(
            l10n.inboxChoiceUnavailable,
            style: text.caption.copyWith(color: colors.text2),
          ),
        ],
      ],
    );
  }
}

/// "Create person…" for an entity link-or-create suggestion: the person's
/// name, with the mention as an alias. Saves with `CoreApi.createEntity`
/// (`kind: person`, `aliases: [mention]`, duplicate-checked; the candidates
/// are shown in place and Create anyway resends with `force: true`), then
/// accepts the suggestion (`CoreApi.acceptSuggestion`).
class CreatePersonSheet extends HookConsumerWidget {
  /// Creates the sheet for [suggestion].
  const new({required this.suggestion, super.key, this.onOpenEntity});

  /// The link-or-create suggestion.
  final SuggestionItem suggestion;

  /// Opens an existing person from the duplicate check.
  final ValueChanged<String>? onOpenEntity;

  /// Presents the sheet (compact) or dialog.
  static Future<void> show(
    BuildContext context, {
    required SuggestionItem suggestion,
    ValueChanged<String>? onOpenEntity,
  }) {
    final sheet = CreatePersonSheet(
      suggestion: suggestion,
      onOpenEntity: onOpenEntity,
    );
    if (SizeClass.of(context) == SizeClass.compact) {
      return showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        builder: (_) => sheet,
      );
    }
    return showDialog<void>(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480),
          child: sheet,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final name = useTextEditingController();
    final value = useValueListenable(name);
    final candidates = useState<List<CandidateItem>?>(null);
    final mention = suggestion.detail.mention;

    Future<void> create({required bool force}) async {
      final api = ref.read(coreApiProvider);
      final messenger = ScaffoldMessenger.maybeOf(context);
      final l10n = lookupInboxLocalizations(Localizations.localeOf(context));
      try {
        final outcome = await api.createEntity(
          kind: 'person',
          name: name.text,
          aliases: [mention],
          force: force,
        );
        if (outcome.id == null) {
          candidates.value = outcome.candidates;
          return;
        }
        await api.acceptSuggestion(id: suggestion.id);
        if (context.mounted) await Navigator.maybePop(context);
      } on Object catch (error) {
        messenger?.showSnackBar(
          SnackBar(
            content: Text(
              error is CoreFailure
                  ? l10n.inboxActionFailedCode(code: error.code)
                  : l10n.inboxActionFailed,
            ),
          ),
        );
      }
    }

    return InboxL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.inboxL10n;
          final text = context.strataText;
          final found = candidates.value;
          if (found != null) {
            return SingleChildScrollView(
              child: DuplicateCandidatesView(
                kind: 'person',
                title: name.text,
                candidates: found,
                onOpenExisting: (candidate) {
                  onOpenEntity?.call(candidate.id);
                  Navigator.maybePop(context);
                },
                onCreateAnyway: () => unawaited(create(force: true)),
                onCancel: () => candidates.value = null,
              ),
            );
          }
          return SingleChildScrollView(
            padding: EdgeInsets.fromLTRB(
              StrataSpacing.s5,
              StrataSpacing.s4,
              StrataSpacing.s5,
              StrataSpacing.s5 + MediaQuery.viewInsetsOf(context).bottom,
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Semantics(
                  header: true,
                  container: true,
                  child: Text(l10n.inboxCreatePersonTitle, style: text.title),
                ),
                const SizedBox(height: StrataSpacing.s4),
                TextField(
                  controller: name,
                  autofocus: true,
                  decoration: InputDecoration(labelText: l10n.inboxPersonName),
                ),
                const SizedBox(height: StrataSpacing.s3),
                ProposalLabel(l10n.inboxAliases),
                Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: StatusPill(
                    label: mention,
                    icon: Icons.alternate_email,
                  ),
                ),
                const SizedBox(height: StrataSpacing.s2),
                Text(
                  l10n.inboxAliasNote(mention: mention),
                  style: text.caption.copyWith(
                    color: context.strataColors.text2,
                  ),
                ),
                const SizedBox(height: StrataSpacing.s5),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: StrataSpacing.s2,
                  children: [
                    TextButton(
                      onPressed: () => Navigator.maybePop(context),
                      child: Text(
                        l10n.inboxCancel,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    FilledButton(
                      style: tallFilledButton,
                      onPressed: value.text.isEmpty
                          ? null
                          : () => unawaited(create(force: false)),
                      child: Text(
                        l10n.inboxCreatePersonSave,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}
