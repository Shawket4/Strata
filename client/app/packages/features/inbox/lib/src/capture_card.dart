import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_inbox/src/l10n.dart';
import 'package:strata_inbox/src/proposals.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// Accepts every suggestion of [item], in the core's order
/// (`CoreApi.acceptSuggestion` per suggestion).
Future<void> acceptCapture(
  BuildContext context,
  CoreApi api,
  InboxItem item,
) async {
  for (final suggestion in item.suggestions) {
    await forwardIntent(context, api.acceptSuggestion(id: suggestion.id));
    if (!context.mounted) return;
  }
}

/// Rejects every suggestion of [item] (`CoreApi.rejectSuggestion`).
Future<void> rejectCapture(
  BuildContext context,
  CoreApi api,
  InboxItem item,
) async {
  for (final suggestion in item.suggestions) {
    await forwardIntent(context, api.rejectSuggestion(id: suggestion.id));
    if (!context.mounted) return;
  }
}

/// The captured text, direction-neutral (the core gives no language hint
/// yet): aligned to the start of the line.
class CaptureText extends StatelessWidget {
  /// Creates the text.
  const new({required this.text, super.key, this.maxLines});

  /// The capture body as the core streams it.
  final String text;

  /// Clamp for list rows.
  final int? maxLines;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      textAlign: TextAlign.start,
      maxLines: maxLines,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
      style: context.strataText.body,
    );
  }
}

/// A capture with the AI's proposal and Reject / Edit / Accept.
///
/// Accept and Reject forward every suggestion of the capture; Edit opens the
/// capture note (editing the proposal itself needs a core intent,
/// docs/CORE_GAPS.md).
class CaptureCard extends ConsumerWidget {
  /// Creates the card.
  const new({
    required this.item,
    super.key,
    this.density = ProposalDensity.card,
    this.onOpenNote,
    this.onOpenEntity,
    this.showText = true,
  });

  /// The capture.
  final InboxItem item;

  /// Card (compact list) or detail pane.
  final ProposalDensity density;

  /// Opens a note by id.
  final ValueChanged<String>? onOpenNote;

  /// Opens an entity by id.
  final ValueChanged<String>? onOpenEntity;

  /// Whether the capture text is shown (the expanded detail shows it in its
  /// own column).
  final bool showText;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final api = ref.read(coreApiProvider);
    final open = onOpenNote;
    final hasProposal = item.suggestions.isNotEmpty;
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showText) ...[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(child: CaptureText(text: item.text)),
              if (item.pendingSync) ...[
                const SizedBox(width: StrataSpacing.s2),
                const NotSyncedMarker(),
              ],
            ],
          ),
          const SizedBox(height: StrataSpacing.s3),
        ],
        if (!hasProposal)
          Row(
            children: [
              Icon(Icons.hourglass_empty, size: 16, color: colors.text2),
              const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: Text(
                  l10n.inboxNoProposalYet,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              ),
            ],
          )
        else ...[
          Row(
            children: [
              Icon(
                Icons.auto_awesome_outlined,
                size: 16,
                color: colors.accentText,
              ),
              const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.inboxAiProposal,
                    style: text.caption
                        .withWeight(FontWeight.w700)
                        .copyWith(color: colors.accentText),
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s2),
          for (final suggestion in item.suggestions)
            Padding(
              padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
              child: ProposalContent(
                suggestion: suggestion,
                density: density,
                onOpenNote: onOpenNote,
                onOpenEntity: onOpenEntity,
              ),
            ),
        ],
        const SizedBox(height: StrataSpacing.s2),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: StrataSpacing.s2,
          runSpacing: StrataSpacing.s2,
          children: [
            TextButton(
              onPressed: hasProposal
                  ? () => unawaited(rejectCapture(context, api, item))
                  : null,
              child: Text(l10n.inboxReject),
            ),
            OutlinedButton(
              onPressed: open == null ? null : () => open(item.noteId),
              child: Text(l10n.inboxEdit),
            ),
            FilledButton(
              onPressed: hasProposal
                  ? () => unawaited(acceptCapture(context, api, item))
                  : null,
              child: Text(l10n.inboxAccept),
            ),
          ],
        ),
      ],
    );
    if (density == ProposalDensity.detail) return content;
    return Semantics(
      container: true,
      label: l10n.inboxCaptureSemantics,
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
