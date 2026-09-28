import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/sections.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The directory's suggestion strip: the tab's entity suggestions from the
/// core (`DirectoryView.suggestions`: who-is, duplicates, custody), each
/// answered in place.
class SuggestionStrip extends StatelessWidget {
  /// Creates the strip.
  const new({required this.suggestions, super.key, this.columns = 1});

  /// The suggestions, in the core's order.
  final List<SuggestionItem> suggestions;

  /// Cards per row.
  final int columns;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    if (suggestions.isEmpty) return const SizedBox();
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

/// One suggestion: what it proposes (by kind) and its answers — a who-is
/// links the mention to a candidate or creates the entity
/// (`resolve_link_or_create`), an ambiguous custody update picks its
/// document (`accept_suggestion_choice`), others are accepted as proposed;
/// Dismiss rejects it.
class SuggestionCard extends ConsumerWidget {
  /// Creates the card for [item].
  const new({required this.item, super.key});

  /// The suggestion.
  final SuggestionItem item;

  Future<void> _run(
    BuildContext context,
    Future<Object?> Function() intent,
  ) async {
    final l10n = context.dirL10n;
    final messenger = ScaffoldMessenger.maybeOf(context);
    try {
      await intent();
    } on Object catch (error) {
      messenger
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(dirFailure(l10n, error))));
    }
  }

  Future<void> _create(
    BuildContext context,
    WidgetRef ref, {
    required bool force,
  }) async {
    final l10n = context.dirL10n;
    final messenger = ScaffoldMessenger.maybeOf(context);
    final detail = item.detail;
    try {
      final outcome = await ref
          .read(coreApiProvider)
          .resolveLinkOrCreate(
            id: item.id,
            choice: LinkOrCreateChoice(
              kind: LinkOrCreateKind.create,
              name: detail.mention,
              entityKind: detail.entityKind.isEmpty ? null : detail.entityKind,
              force: force,
            ),
          );
      if (outcome.id == null && outcome.candidates.isNotEmpty) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(
              content: Text(
                l10n.createExists(title: outcome.candidates.first.title),
              ),
              action: SnackBarAction(
                label: l10n.createAnyway,
                onPressed: () {
                  if (context.mounted) {
                    unawaited(_create(context, ref, force: true));
                  }
                },
              ),
            ),
          );
      }
    } on Object catch (error) {
      messenger
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(dirFailure(l10n, error))));
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final core = ref.read(coreApiProvider);
    final detail = item.detail;
    final (icon, title, body) = switch (detail.kind) {
      SuggestionKind.entityLink => (
        Icons.person_search_outlined,
        l10n.whoIs(mention: detail.mention),
        detail.reason.isNotEmpty
            ? detail.reason
            : l10n.whoIsCandidates(count: detail.candidates.length),
      ),
      SuggestionKind.duplicate || SuggestionKind.duplicates => (
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
    final source = item.sourceText;
    final answers = <Widget>[
      if (detail.kind == SuggestionKind.entityLink) ...[
        for (final candidate in detail.candidates)
          if (candidate.id != null)
            FilledButton.tonal(
              onPressed: () => _run(
                context,
                () => core.resolveLinkOrCreate(
                  id: item.id,
                  choice: LinkOrCreateChoice(
                    kind: LinkOrCreateKind.link,
                    entityId: candidate.id,
                    force: false,
                  ),
                ),
              ),
              child: Text(l10n.linkTo(title: candidate.title)),
            ),
        OutlinedButton(
          onPressed: () => _create(context, ref, force: false),
          child: Text(
            detail.entityKind == 'company'
                ? l10n.createCompany
                : l10n.createPerson,
          ),
        ),
      ] else if (detail.kind == SuggestionKind.custody &&
          detail.documentChoices.isNotEmpty) ...[
        for (final document in detail.documentChoices)
          if (document.id != null)
            FilledButton.tonal(
              onPressed: () => _run(
                context,
                () => core.acceptSuggestionChoice(
                  id: item.id,
                  documentId: document.id!,
                ),
              ),
              child: Text(document.title),
            ),
      ] else
        FilledButton.tonal(
          onPressed: item.canAccept
              ? () => _run(context, () => core.acceptSuggestion(id: item.id))
              : null,
          child: Text(l10n.accept),
        ),
    ];
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
                      if (source != null && source.isNotEmpty)
                        Text(
                          source,
                          textDirection: textDirectionOf(item.sourceDir),
                          textAlign: TextAlign.start,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                      if (detail.duplicates.isNotEmpty)
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
                      Wrap(
                        spacing: StrataSpacing.s2,
                        children: [
                          if (confidence != null)
                            Text(
                              l10n.aiConfidence(
                                value: confidence.toStringAsFixed(2),
                              ),
                              style: text.caption.copyWith(
                                color: colors.infoText,
                              ),
                            ),
                          if (item.createdLabel.isNotEmpty)
                            Text(
                              item.createdLabel,
                              style: text.caption.copyWith(color: colors.text2),
                            ),
                        ],
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
                  onPressed: () =>
                      _run(context, () => core.rejectSuggestion(id: item.id)),
                  child: Text(l10n.dismiss),
                ),
                ...answers,
              ],
            ),
          ],
        ),
      ),
    );
  }
}
