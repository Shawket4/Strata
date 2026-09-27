import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/async_body.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_ui/strata_ui.dart';

/// The "Already exists" content shared by every create flow: what is being
/// created, the candidates the core found (best first, with match level and
/// score) and Open existing / Create anyway / Cancel.
class DuplicateCandidatesView extends StatelessWidget {
  /// Creates the view.
  const new({
    required this.kind,
    required this.title,
    required this.candidates,
    required this.onOpenExisting,
    required this.onCreateAnyway,
    required this.onCancel,
    super.key,
  });

  /// Kind of the item being created (`task`, `note`, `person`, …).
  final String kind;

  /// Title of the item being created.
  final String title;

  /// Candidates, best first (as the core orders them).
  final List<CandidateItem> candidates;

  /// Opens an existing candidate.
  final ValueChanged<CandidateItem> onOpenExisting;

  /// Creates the item anyway.
  final VoidCallback onCreateAnyway;

  /// Does not create it.
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          final colors = context.strataColors;
          final text = context.strataText;
          final best = candidates.isEmpty ? null : candidates.first;
          return Padding(
            padding: const EdgeInsets.all(StrataSpacing.s5),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Container(
                      width: 36,
                      height: 36,
                      decoration: BoxDecoration(
                        color: colors.warningTint,
                        shape: BoxShape.circle,
                      ),
                      child: Icon(
                        Icons.content_copy_outlined,
                        size: 18,
                        color: colors.warningText,
                      ),
                    ),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(
                      child: Text(
                        l10n.dupEyebrow,
                        style: text.caption
                            .withWeight(FontWeight.w700)
                            .copyWith(
                              color: colors.warningText,
                              letterSpacing: 0.6,
                            ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s3),
                Semantics(
                  header: true,
                  child: Text(l10n.dupTitle, style: text.titleSmall),
                ),
                const SizedBox(height: StrataSpacing.s1),
                Text(
                  l10n.dupSubtitle(
                    kind: CoreLabels.itemKind(l10n, kind),
                    title: title,
                  ),
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
                const SizedBox(height: StrataSpacing.s4),
                for (final candidate in candidates) ...[
                  _CandidateCard(
                    candidate: candidate,
                    onOpen: () => onOpenExisting(candidate),
                  ),
                  const SizedBox(height: StrataSpacing.s2),
                ],
                const SizedBox(height: StrataSpacing.s3),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: StrataSpacing.s2,
                  runSpacing: StrataSpacing.s2,
                  children: [
                    TextButton(
                      onPressed: onCancel,
                      child: Text(l10n.dupCancel),
                    ),
                    OutlinedButton(
                      onPressed: onCreateAnyway,
                      child: Text(l10n.dupCreateAnyway),
                    ),
                    if (best != null)
                      FilledButton(
                        onPressed: () => onOpenExisting(best),
                        child: Text(l10n.dupOpenExisting),
                      ),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s3),
                Text(
                  l10n.dupFootnote,
                  style: text.caption.copyWith(color: colors.text2),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _CandidateCard extends StatelessWidget {
  const new({required this.candidate, required this.onOpen});

  final CandidateItem candidate;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final kind = CoreLabels.nodeKind(candidate.kind);
    final snippet = candidate.snippet;
    final match = l10n.dupMatch(
      level: CoreLabels.matchLevel(l10n, candidate.matchLevel),
      score: CoreLabels.score(candidate.score),
    );
    final kindLabel = CoreLabels.itemKind(l10n, candidate.kind);
    return Semantics(
      button: true,
      label: l10n.dupCandidateSemantics(
        title: candidate.title,
        kind: kindLabel,
        match: match,
      ),
      excludeSemantics: true,
      child: Material(
        color: colors.surface,
        shape: RoundedRectangleBorder(
          borderRadius: StrataRadii.cardRadius,
          side: BorderSide(color: colors.border),
        ),
        child: InkWell(
          customBorder: const RoundedRectangleBorder(
            borderRadius: StrataRadii.cardRadius,
          ),
          onTap: onOpen,
          child: Padding(
            padding: const EdgeInsets.all(StrataSpacing.s3),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: kind == null
                      ? Icon(
                          Icons.task_alt,
                          size: 18,
                          color: colors.accentText,
                        )
                      : NodeKindGlyph(kind: kind, size: 18, decorative: true),
                ),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        candidate.title,
                        style: text.body.withWeight(FontWeight.w600),
                      ),
                      if (snippet != null)
                        Text(
                          snippet,
                          style: text.bodySmall.copyWith(color: colors.text2),
                        ),
                      const SizedBox(height: StrataSpacing.s1),
                      Wrap(
                        spacing: StrataSpacing.s2,
                        runSpacing: StrataSpacing.s1,
                        children: [
                          StatusPill(
                            label: kindLabel,
                            icon: Icons.label_outline,
                          ),
                          StatusPill(
                            label: match,
                            tone: StatusTone.warning,
                            icon: Icons.compare_arrows,
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The shared "Already exists" sheet / dialog for prompts raised by a push
/// (`duplicatePromptsProvider`): shows the oldest open prompt and answers it
/// with `CoreApi.resolveDuplicate` (Open existing and Cancel send
/// [DuplicateChoice.discard], Create anyway [DuplicateChoice.createAnyway]).
///
/// Use [DuplicatePromptSheet.show] from any create flow: a bottom sheet on
/// compact, a dialog on medium and expanded.
class DuplicatePromptSheet extends ConsumerWidget {
  /// Creates the sheet content.
  const new({super.key, this.onOpenExisting, this.closeOnResolve = false});

  /// Opens an existing candidate (the app shell routes by kind and id).
  final ValueChanged<CandidateItem>? onOpenExisting;

  /// Pops the enclosing route after an answer (set by [show]).
  final bool closeOnResolve;

  /// Presents the sheet (compact) or dialog (medium / expanded).
  static Future<void> show(
    BuildContext context, {
    ValueChanged<CandidateItem>? onOpenExisting,
  }) {
    final sheet = DuplicatePromptSheet(
      onOpenExisting: onOpenExisting,
      closeOnResolve: true,
    );
    if (SizeClass.of(context) == SizeClass.compact) {
      return showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        builder: (_) => SingleChildScrollView(child: sheet),
      );
    }
    return showDialog<void>(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560),
          child: SingleChildScrollView(child: sheet),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final prompts = ref.watch(duplicatePromptsProvider);
    return TasksL10nScope(
      child: Builder(
        builder: (context) => CoreAsyncBody(
          value: prompts,
          errorTitle: context.tasksL10n.dupLoadError,
          data: (view) {
            if (view.prompts.isEmpty) {
              return Padding(
                padding: const EdgeInsets.all(StrataSpacing.s6),
                child: Text(
                  context.tasksL10n.dupNoneOpen,
                  textAlign: TextAlign.center,
                ),
              );
            }
            final prompt = view.prompts.first;
            void resolve(DuplicateChoice choice) {
              final api = ref.read(coreApiProvider);
              unawaited(
                forwardIntent(
                  context,
                  api.resolveDuplicate(opId: prompt.opId, choice: choice),
                ),
              );
              if (closeOnResolve) unawaited(Navigator.maybePop(context));
            }

            return DuplicateCandidatesView(
              kind: prompt.kind,
              title: prompt.title,
              candidates: prompt.candidates,
              onOpenExisting: (candidate) {
                resolve(DuplicateChoice.discard);
                onOpenExisting?.call(candidate);
              },
              onCreateAnyway: () => resolve(DuplicateChoice.createAnyway),
              onCancel: () => resolve(DuplicateChoice.discard),
            );
          },
        ),
      ),
    );
  }
}
