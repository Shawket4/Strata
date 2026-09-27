import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_ask/src/common.dart';
import 'package:strata_ask/src/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Ask (PLAN §11 screen 10): the conversation over the vault from the core's
/// view, answers with tappable citations and their sources, "save as note",
/// scope, and the online-only states (offline, not available yet). Compact
/// and medium: one conversation column; expanded: conversation + source
/// preview panel.
class AskScreen extends StatelessWidget {
  /// Creates Ask.
  const new({super.key, this.onOpenNote});

  /// The icon that represents this feature.
  static const IconData icon = Icons.forum_outlined;

  /// Opens a note at a block (citations).
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context) =>
      AskLocalizationScope(child: _Ask(onOpenNote: onOpenNote));
}

class _Ask extends HookConsumerWidget {
  const new({required this.onOpenNote});

  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colors = context.strataColors;
    final expanded = SizeClass.of(context) == SizeClass.expanded;
    final preview = useState<Citation?>(null);
    final body = switch (ref.watch(askViewProvider)) {
      AsyncData(:final value) => _Conversation(
        view: value,
        onCitation: (citation) {
          if (expanded) {
            preview.value = citation;
          } else if (citation.noteId != null) {
            onOpenNote?.call(citation.noteId!, citation.anchor);
          }
        },
      ),
      AsyncError(:final error) => AskError(error: error),
      _ => const AskLoading(),
    };
    if (!expanded) return ColoredBox(color: colors.background, child: body);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: ColoredBox(color: colors.background, child: body),
        ),
        VerticalDivider(width: 1, color: colors.border),
        SizedBox(
          width: StrataLayout.contextPanelWidth,
          child: _SourcePreview(
            citation: preview.value,
            onOpenNote: onOpenNote,
          ),
        ),
      ],
    );
  }
}

class _Conversation extends HookWidget {
  const new({required this.view, required this.onCitation});

  final AskView view;
  final ValueChanged<Citation> onCitation;

  @override
  Widget build(BuildContext context) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final question = useTextEditingController();
    final banner = switch (view.availability) {
      Availability.available => null as (IconData, String, String?)?,
      Availability.offline => (
        Icons.cloud_off_outlined,
        l10n.offlineTitle,
        l10n.offlineMessage,
      ),
      Availability.notYetAvailable => (
        Icons.hourglass_empty,
        l10n.notYetTitle,
        l10n.notYetMessage,
      ),
      Availability.notAllowed => (Icons.block, l10n.notAllowedTitle, null),
    };
    final shown = banner;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s4,
            StrataSpacing.s2,
            StrataSpacing.s2,
            0,
          ),
          child: Row(
            children: [
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(l10n.askTitle, style: text.titleSmall),
                ),
              ),
              IconButton(
                tooltip: l10n.newConversationUnavailable,
                onPressed: null,
                icon: const Icon(Icons.add_comment_outlined),
              ),
            ],
          ),
        ),
        if (shown != null)
          Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s4,
              vertical: StrataSpacing.s2,
            ),
            child: Semantics(
              liveRegion: true,
              container: true,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: view.availability == Availability.offline
                      ? colors.warningTint
                      : colors.infoTint,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: Padding(
                  padding: const EdgeInsets.all(StrataSpacing.s3),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(
                        shown.$1,
                        size: 20,
                        color: view.availability == Availability.offline
                            ? colors.warningText
                            : colors.infoText,
                      ),
                      const SizedBox(width: StrataSpacing.s2),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              shown.$2,
                              style: text.bodySmall
                                  .withWeight(FontWeight.w700)
                                  .copyWith(
                                    color:
                                        view.availability ==
                                            Availability.offline
                                        ? colors.warningText
                                        : colors.infoText,
                                  ),
                            ),
                            if (shown.$3 != null)
                              Text(
                                shown.$3!,
                                style: text.caption.copyWith(
                                  color:
                                      view.availability == Availability.offline
                                      ? colors.warningText
                                      : colors.infoText,
                                ),
                              ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        Expanded(
          child: view.messages.isEmpty
              ? StrataEmptyState(
                  icon: AskScreenIcon.icon,
                  title: l10n.emptyTitle,
                  message: l10n.emptyMessage,
                )
              : Semantics(
                  label: l10n.conversation,
                  container: true,
                  explicitChildNodes: true,
                  child: ListView(
                    padding: const EdgeInsets.all(StrataSpacing.s4),
                    children: [
                      for (final message in view.messages)
                        Center(
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(maxWidth: 760),
                            child: message.role == 'user'
                                ? _UserMessage(message: message)
                                : _Answer(
                                    message: message,
                                    onCitation: onCitation,
                                  ),
                          ),
                        ),
                    ],
                  ),
                ),
        ),
        _Composer(controller: question),
      ],
    );
  }
}

/// The feature icon (kept separately so widgets below the screen can use it).
abstract final class AskScreenIcon {
  /// Ask's icon.
  static const IconData icon = Icons.forum_outlined;
}

class _UserMessage extends StatelessWidget {
  const new({required this.message});

  final AskMessage message;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Align(
      alignment: AlignmentDirectional.centerEnd,
      child: Semantics(
        label: context.askL10n.you,
        container: true,
        child: Container(
          margin: const EdgeInsets.only(bottom: StrataSpacing.s4),
          constraints: const BoxConstraints(maxWidth: 560),
          padding: const EdgeInsets.symmetric(
            horizontal: StrataSpacing.s4,
            vertical: StrataSpacing.s3,
          ),
          decoration: BoxDecoration(
            color: colors.accentTint,
            borderRadius: StrataRadii.cardRadius,
          ),
          child: Text(
            message.text,
            textAlign: TextAlign.start,
            style: context.strataText.body,
          ),
        ),
      ),
    );
  }
}

class _Answer extends StatelessWidget {
  const new({required this.message, required this.onCitation});

  final AskMessage message;
  final ValueChanged<Citation> onCitation;

  @override
  Widget build(BuildContext context) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final citations = message.citations;
    return Semantics(
      label: l10n.answer,
      container: true,
      explicitChildNodes: true,
      child: Padding(
        padding: const EdgeInsets.only(bottom: StrataSpacing.s6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(message.text, textAlign: TextAlign.start, style: text.body),
            if (citations.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s2),
              Wrap(
                spacing: StrataSpacing.s1,
                runSpacing: StrataSpacing.s1,
                children: [
                  for (var i = 0; i < citations.length; i++)
                    CitationChip(
                      index: i + 1,
                      label: citations[i].target,
                      blockRef: citations[i].anchor,
                      onPressed: () => onCitation(citations[i]),
                    ),
                ],
              ),
              const SizedBox(height: StrataSpacing.s3),
              Semantics(
                header: true,
                child: Text(
                  l10n.sources(count: citations.length),
                  style: text.caption
                      .withWeight(FontWeight.w700)
                      .copyWith(color: colors.text2),
                ),
              ),
              Card(
                child: Column(
                  children: [
                    for (var i = 0; i < citations.length; i++)
                      ListTile(
                        dense: true,
                        leading: Text(
                          '${i + 1}',
                          style: text.monoSmall.copyWith(color: colors.text2),
                        ),
                        title: Text(citations[i].target),
                        trailing: citations[i].anchor == null
                            ? null
                            : Text(
                                citations[i].anchor!,
                                textDirection: TextDirection.ltr,
                                style: text.monoSmall.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                        onTap: () => onCitation(citations[i]),
                      ),
                  ],
                ),
              ),
            ],
            const SizedBox(height: StrataSpacing.s2),
            Wrap(
              spacing: StrataSpacing.s2,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Tooltip(
                  message: l10n.saveAsNoteUnavailable,
                  child: OutlinedButton.icon(
                    onPressed: null,
                    icon: const Icon(Icons.note_add_outlined, size: 18),
                    label: Text(l10n.saveAsNote),
                    style: OutlinedButton.styleFrom(
                      disabledForegroundColor: colors.text2,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: l10n.copyAnswer,
                  onPressed: () async {
                    await Clipboard.setData(ClipboardData(text: message.text));
                    if (!context.mounted) return;
                    ScaffoldMessenger.maybeOf(context)
                        ?.showSnackBar(SnackBar(content: Text(l10n.copied)));
                  },
                  icon: const Icon(Icons.copy_outlined, size: 18),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _Composer extends StatelessWidget {
  const new({required this.controller});

  final TextEditingController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.surface,
        border: Border(top: BorderSide(color: colors.border)),
      ),
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.all(StrataSpacing.s3),
          child: Semantics(
            label: l10n.question,
            container: true,
            explicitChildNodes: true,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Wrap(
                  spacing: StrataSpacing.s2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      l10n.scope,
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                    Semantics(
                      label: l10n.scope,
                      container: true,
                      explicitChildNodes: true,
                      child: ChoiceChip(
                        label: Text(l10n.scopeAll),
                        selected: true,
                        onSelected: (_) {},
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s2),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Expanded(
                      child: TextField(
                        controller: controller,
                        minLines: 1,
                        maxLines: 4,
                        decoration: InputDecoration(
                          hintText: l10n.askHint,
                          helperText: l10n.sendUnavailable,
                          helperMaxLines: 2,
                          helperStyle: text.caption.copyWith(
                            color: colors.text2,
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(width: StrataSpacing.s2),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 22),
                      child: IconButton.filled(
                        tooltip: l10n.send,
                        onPressed: null,
                        icon: const Icon(Icons.arrow_upward),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SourcePreview extends ConsumerWidget {
  const new({required this.citation, required this.onOpenNote});

  final Citation? citation;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final shown = citation;
    final noteId = shown?.noteId;
    final note = noteId == null
        ? null
        : ref.watch(noteProvider(noteId)).value?.note;
    final open = onOpenNote;
    return Semantics(
      label: l10n.sourcePreview,
      container: true,
      explicitChildNodes: true,
      child: ColoredBox(
        color: colors.surface,
        child: ListView(
          padding: const EdgeInsets.all(StrataSpacing.s4),
          children: [
            Semantics(
              header: true,
              child: Text(
                l10n.sourcePreview,
                style: text.bodySmall.withWeight(FontWeight.w700),
              ),
            ),
            const SizedBox(height: StrataSpacing.s3),
            if (shown == null)
              Text(
                l10n.sourcePreviewHint,
                style: text.bodySmall.copyWith(color: colors.text2),
              )
            else ...[
              Text(note?.title ?? shown.target, style: text.titleSmall),
              if (note != null)
                Text(
                  note.path,
                  textDirection: TextDirection.ltr,
                  style: text.monoSmall.copyWith(color: colors.text2),
                ),
              if (noteId == null)
                Text(
                  l10n.noteMissing,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              if (note != null && note.tags.isNotEmpty) ...[
                const SizedBox(height: StrataSpacing.s2),
                Wrap(
                  spacing: StrataSpacing.s1,
                  runSpacing: StrataSpacing.s1,
                  children: [
                    for (final tag in note.tags) Chip(label: Text(tag)),
                  ],
                ),
              ],
              if (shown.anchor != null) ...[
                const SizedBox(height: StrataSpacing.s2),
                Text(
                  shown.anchor!,
                  textDirection: TextDirection.ltr,
                  style: text.monoSmall.copyWith(color: colors.text2),
                ),
              ],
              const SizedBox(height: StrataSpacing.s3),
              if (open != null && noteId != null)
                FilledButton.icon(
                  onPressed: () => open(noteId, shown.anchor),
                  icon: const Icon(Icons.open_in_new, size: 18),
                  label: Text(l10n.openAtBlock),
                ),
            ],
          ],
        ),
      ),
    );
  }
}
