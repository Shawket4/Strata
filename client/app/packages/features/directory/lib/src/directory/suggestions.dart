import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The directory's suggestion strip: the core's pending suggestions that are
/// not tied to an inbox capture (who-is, merge / duplicate, custody, …),
/// each with Accept / Dismiss.
class SuggestionStrip extends ConsumerWidget {
  /// Creates the strip.
  const new({super.key, this.columns = 1});

  /// Cards per row.
  final int columns;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final suggestions = ref.watch(inboxProvider).value?.suggestions;
    if (suggestions == null || suggestions.isEmpty) return const SizedBox();
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: StrataSpacing.s2,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Icon(Icons.auto_awesome_outlined, size: 18, color: colors.text2),
              Semantics(
                header: true,
                child: Text(
                  l10n.suggestions,
                  style: text.bodySmall.withWeight(FontWeight.w700),
                ),
              ),
              Text(
                l10n.suggestionsHint,
                style: text.caption.copyWith(color: colors.text2),
              ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s2),
          LayoutBuilder(
            builder: (context, constraints) {
              const gap = StrataSpacing.s3;
              final width = columns == 1
                  ? constraints.maxWidth
                  : (constraints.maxWidth - gap * (columns - 1)) / columns;
              return Wrap(
                spacing: gap,
                runSpacing: gap,
                children: [
                  for (final item in suggestions)
                    SizedBox(
                      width: width,
                      child: SuggestionCard(item: item),
                    ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }
}

/// One suggestion: what it proposes (by kind) and Accept / Dismiss.
class SuggestionCard extends ConsumerWidget {
  /// Creates the card for [item].
  const new({required this.item, super.key});

  /// The suggestion.
  final SuggestionItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = item.detail;
    final (icon, title, body) = switch (detail.kind) {
      SuggestionKind.entityLink => (
        Icons.person_search_outlined,
        l10n.whoIs(mention: detail.mention),
        l10n.whoIsCandidates(count: detail.candidates.length),
      ),
      SuggestionKind.duplicate => (
        Icons.merge_type,
        l10n.possibleDuplicate,
        null,
      ),
      SuggestionKind.custody => (
        Icons.swap_horiz,
        l10n.custodySuggestion,
        detail.line,
      ),
      SuggestionKind.correction => (
        Icons.hub_outlined,
        l10n.relationSuggestion,
        detail.reason,
      ),
      SuggestionKind.task => (
        Icons.task_alt_outlined,
        l10n.taskSuggestion,
        detail.line,
      ),
      SuggestionKind.filing => (
        Icons.folder_outlined,
        l10n.filingSuggestion,
        detail.title,
      ),
      _ => (Icons.lightbulb_outline, l10n.otherSuggestion, null),
    };
    final confidence = detail.confidence;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                  width: 32,
                  height: 32,
                  decoration: BoxDecoration(
                    color: colors.accentTint,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(icon, size: 18, color: colors.accentText),
                ),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: text.bodySmall.withWeight(FontWeight.w600),
                      ),
                      if (body != null && body.isNotEmpty)
                        Text(
                          body,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                      if (detail.kind == SuggestionKind.duplicate)
                        Wrap(
                          spacing: StrataSpacing.s1,
                          children: [
                            for (final candidate in detail.duplicates)
                              Text(
                                candidate.title,
                                style: text.caption.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                          ],
                        ),
                      if (confidence != null)
                        Text(
                          l10n.aiConfidence(
                            value: confidence.toStringAsFixed(2),
                          ),
                          style: text.caption.copyWith(color: colors.infoText),
                        ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: StrataSpacing.s2),
            Wrap(
              alignment: WrapAlignment.end,
              spacing: StrataSpacing.s2,
              runSpacing: StrataSpacing.s1,
              children: [
                TextButton(
                  onPressed: () => unawaited(
                    ref.read(coreApiProvider).rejectSuggestion(id: item.id),
                  ),
                  child: Text(l10n.dismiss),
                ),
                FilledButton.tonal(
                  onPressed: () => unawaited(
                    ref.read(coreApiProvider).acceptSuggestion(id: item.id),
                  ),
                  child: Text(
                    detail.kind == SuggestionKind.entityLink &&
                            detail.candidates.isEmpty
                        ? l10n.createPerson
                        : l10n.accept,
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
