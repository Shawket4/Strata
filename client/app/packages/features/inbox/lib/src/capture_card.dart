import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_inbox/src/l10n.dart';
import 'package:strata_inbox/src/proposals.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// Accepts a capture's proposal (`accept_capture`: every pending
/// suggestion that needs no choice).
Future<void> acceptCapture(BuildContext context, CoreApi api, InboxItem item) =>
    forwardIntent(context, api.acceptCapture(noteId: item.noteId));

/// Rejects a capture's proposal (`reject_capture`).
Future<void> rejectCapture(BuildContext context, CoreApi api, InboxItem item) =>
    forwardIntent(context, api.rejectCapture(noteId: item.noteId));

/// The captured text in the direction the core detected (`text_dir`),
/// aligned to the start of the line.
class CaptureText extends StatelessWidget {
  /// Creates the text.
  const new({
    required this.text,
    super.key,
    this.maxLines,
    this.dir = TextDir.neutral,
  });

  /// The capture body as the core streams it.
  final String text;

  /// Clamp for list rows.
  final int? maxLines;

  /// Its direction.
  final TextDir dir;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      textDirection: textDirectionOf(dir),
      textAlign: TextAlign.start,
      maxLines: maxLines,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
      style: context.strataText.body,
    );
  }
}

/// A capture with its metadata (when, where from, ready / needs you /
/// duplicate), the AI's proposal and Reject / Edit / Accept: Accept and
/// Reject act on the capture (`accept_capture` / `reject_capture`); Edit
/// changes the proposal before accepting it (`accept_suggestion_with`), or
/// opens the capture note when there is nothing to edit.
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
    SuggestionItem? editable;
    for (final suggestion in item.suggestions) {
      if (suggestion.detail.kind == SuggestionKind.filing ||
          suggestion.detail.kind == SuggestionKind.task) {
        editable = suggestion;
        break;
      }
    }
    final toEdit = editable;
    final source = item.sourceLabel;
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showText) ...[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: CaptureText(text: item.text, dir: item.textDir),
              ),
              if (item.pendingSync) ...[
                const SizedBox(width: StrataSpacing.s2),
                const NotSyncedMarker(),
              ],
            ],
          ),
          const SizedBox(height: StrataSpacing.s1),
        ],
        Wrap(
          spacing: StrataSpacing.s2,
          runSpacing: StrataSpacing.s1,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            if (item.createdLabel.isNotEmpty)
              Text(
                item.createdLabel,
                style: text.caption.copyWith(color: colors.text2),
              ),
            if (source != null)
              Text(source, style: text.caption.copyWith(color: colors.text2)),
            if (item.ready)
              StatusPill(
                label: l10n.inboxReady,
                tone: StatusTone.success,
                icon: Icons.check,
              ),
            if (item.needsYou)
              StatusPill(
                label: l10n.inboxNeedsYou,
                tone: StatusTone.warning,
                icon: Icons.front_hand_outlined,
              ),
            if (item.isDuplicate)
              StatusPill(
                label: l10n.inboxPossibleDuplicate,
                tone: StatusTone.warning,
                icon: Icons.content_copy_outlined,
              ),
          ],
        ),
        const SizedBox(height: StrataSpacing.s3),
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
                  container: true,
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
              child: Text(
                l10n.inboxReject,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            OutlinedButton(
              onPressed: toEdit != null
                  ? () => unawaited(
                      EditProposalSheet.show(context, suggestion: toEdit),
                    )
                  : open == null
                  ? null
                  : () => open(item.noteId),
              child: Text(
                l10n.inboxEdit,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            FilledButton(
              style: tallFilledButton,
              onPressed: hasProposal
                  ? () => unawaited(acceptCapture(context, api, item))
                  : null,
              child: Text(
                l10n.inboxAccept,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
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
