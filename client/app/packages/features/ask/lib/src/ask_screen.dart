import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_ask/src/common.dart';
import 'package:strata_ask/src/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Ask (PLAN §11 screen 10): the conversation over the vault streamed from
/// the core (`watch_ask`): answers with inline citation markers and their
/// sources, Stop while streaming, "Save as note", the scope, the AI status,
/// and the online-only states (offline, not available yet). Compact and
/// medium: one conversation column (a citation opens the note at its
/// block); expanded: conversation + source preview panel
/// (`resolve_citation`).
class AskScreen extends StatelessWidget {
  /// Creates Ask.
  const new({super.key, this.onOpenNote});

  /// The icon that represents this feature.
  static const IconData icon = Icons.forum_outlined;

  /// Opens a note at a block (citations, saved answers).
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context) =>
      AskLocalizationScope(child: _Ask(onOpenNote: onOpenNote));
}

/// A cited block: the note and the block anchor.
typedef _Cited = ({String noteId, String? anchor});

class _Ask extends HookConsumerWidget {
  const new({required this.onOpenNote});

  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colors = context.strataColors;
    final expanded = SizeClass.of(context) == SizeClass.expanded;
    final preview = useState<_Cited?>(null);
    void cite(_Cited cited) {
      if (expanded) {
        preview.value = cited;
      } else {
        onOpenNote?.call(cited.noteId, cited.anchor);
      }
    }

    final body = switch (ref.watch(askConversationProvider)) {
      AsyncData(:final value) => _Conversation(
        view: value,
        onCitation: cite,
        onOpenNote: onOpenNote,
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
          child: _SourcePreview(cited: preview.value, onOpenNote: onOpenNote),
        ),
      ],
    );
  }
}

/// Runs an Ask intent and reports a failure in a snack bar.
Future<bool> _run(
  BuildContext context,
  Future<Object?> Function() intent,
) async {
  final l10n = context.askL10n;
  final messenger = ScaffoldMessenger.maybeOf(context);
  try {
    await intent();
    return true;
  } on Object catch (error) {
    messenger
      ?..hideCurrentSnackBar()
      ..showSnackBar(
        SnackBar(
          content: Text(
            l10n.errorMessage(
              code: error is CoreFailure ? error.code : 'internal',
            ),
          ),
        ),
      );
    return false;
  }
}

class _Conversation extends HookConsumerWidget {
  const new({
    required this.view,
    required this.onCitation,
    required this.onOpenNote,
  });

  final AskView view;
  final ValueChanged<_Cited> onCitation;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final core = ref.read(coreApiProvider);
    final question = useTextEditingController();
    final scope = useState(0);
    final status = view.aiStatus;
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
    final bannerTone = view.availability == Availability.offline
        ? StatusTone.warning
        : StatusTone.info;
    final bannerColors = bannerTone.colorsIn(colors);
    final bannerWidget = shown == null
        ? null
        : Padding(
            padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
            child: Semantics(
              liveRegion: true,
              container: true,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: bannerColors.background,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: Padding(
                  padding: const EdgeInsets.all(StrataSpacing.s3),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(shown.$1, size: 20, color: bannerColors.foreground),
                      const SizedBox(width: StrataSpacing.s2),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              shown.$2,
                              style: text.bodySmall
                                  .withWeight(FontWeight.w700)
                                  .copyWith(color: bannerColors.foreground),
                            ),
                            if (shown.$3 case final message?)
                              Text(
                                message,
                                style: text.caption.copyWith(
                                  color: bannerColors.foreground,
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
          );

    final note = view.note;

    Future<void> send() async {
      final value = question.text.trim();
      if (value.isEmpty || (note == null && view.scopes.isEmpty)) return;
      // About a note, the question goes to that note's thread.
      final chosen = note != null
          ? AskScope(
              kind: AskScopeKind.note,
              value: note.noteId,
              label: note.title,
            )
          : view.scopes[scope.value.clamp(0, view.scopes.length - 1)];
      final sent = await _run(
        context,
        () => core.ask(question: value, scope: chosen),
      );
      if (sent) question.clear();
    }

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
              Semantics(
                header: true,
                child: Text(l10n.askTitle, style: text.titleSmall),
              ),
              const SizedBox(width: StrataSpacing.s3),
              if (status != null)
                Flexible(
                  child: Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: StatusPill(
                      icon: Icons.auto_awesome_outlined,
                      tone: status.pausedLabel == null
                          ? StatusTone.info
                          : StatusTone.warning,
                      label: status.pausedLabel ?? status.budgetLabel,
                    ),
                  ),
                )
              else
                const Spacer(),
              IconButton(
                tooltip: l10n.newConversation,
                onPressed:
                    (view.messages.isEmpty && note == null) || view.streaming
                    ? null
                    : () => unawaited(_run(context, core.newConversation)),
                icon: const Icon(Icons.add_comment_outlined),
              ),
            ],
          ),
        ),
        if (note != null)
          Padding(
            padding: const EdgeInsetsDirectional.fromSTEB(
              StrataSpacing.s4,
              StrataSpacing.s1,
              StrataSpacing.s4,
              0,
            ),
            child: Align(
              alignment: AlignmentDirectional.centerStart,
              child: InputChip(
                avatar: const Icon(Icons.description_outlined, size: 18),
                label: Text(note.label),
                tooltip: l10n.openNote,
                onPressed: switch (onOpenNote) {
                  final open? => () => open(note.noteId, null),
                  null => null,
                },
                deleteButtonTooltipMessage: l10n.leaveNoteThread,
                onDeleted: view.streaming
                    ? null
                    : () => unawaited(_run(context, core.newConversation)),
              ),
            ),
          ),
        Expanded(
          child: Semantics(
            label: l10n.conversation,
            container: true,
            explicitChildNodes: true,
            child: ListView(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              children: [
                ?bannerWidget,
                if (view.messages.isEmpty)
                  StrataEmptyState(
                    icon: AskScreenIcon.icon,
                    title: l10n.emptyTitle,
                    message: l10n.emptyMessage,
                  ),
                for (final message in view.messages)
                  Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 760),
                      child: message.role == 'user'
                          ? _UserMessage(message: message)
                          : _Answer(
                              message: message,
                              onCitation: onCitation,
                              onOpenNote: onOpenNote,
                            ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        _Composer(
          controller: question,
          scopes: note == null ? view.scopes : const [],
          hint: note == null ? l10n.askHint : l10n.askAboutNoteHint,
          scope: scope.value,
          onScope: (value) => scope.value = value,
          enabled: view.availability == Availability.available,
          streaming: view.streaming,
          onSend: () => unawaited(send()),
          onStop: () => unawaited(_run(context, core.stopAsk)),
        ),
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
            textDirection: textDirectionOf(message.dir),
            textAlign: TextAlign.start,
            style: context.strataText.body,
          ),
        ),
      ),
    );
  }
}

/// Why an answer stopped early (`error_key`).
String _stopReason(AskLocalizations l10n, String key) => switch (key) {
  'stopped' => l10n.answerStopped,
  'error.ai_paused' => l10n.answerPaused,
  'error.ai_unavailable' => l10n.answerUnavailable,
  _ => l10n.answerFailed,
};

class _Answer extends ConsumerWidget {
  const new({
    required this.message,
    required this.onCitation,
    required this.onOpenNote,
  });

  final AskMessage message;
  final ValueChanged<_Cited> onCitation;
  final OpenNoteAt? onOpenNote;

  /// The block of 1-based citation [index] of this answer.
  _Cited? _cited(int index) {
    if (index < 1 || index > message.citations.length) return null;
    final citation = message.citations[index - 1];
    final noteId = citation.noteId;
    return noteId == null ? null : (noteId: noteId, anchor: citation.anchor);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final core = ref.read(coreApiProvider);
    final sources = message.sources;
    final saved = message.savedNoteId;
    final error = message.errorKey;
    final open = onOpenNote;

    Future<void> save() async {
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        final id = await core.saveAnswerAsNote(messageId: message.id);
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(
              content: Text(l10n.savedAsNote),
              action: open == null
                  ? null
                  : SnackBarAction(
                      label: l10n.openNote,
                      onPressed: () => open(id, null),
                    ),
            ),
          );
      } on Object catch (error) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(
              content: Text(
                l10n.errorMessage(
                  code: error is CoreFailure ? error.code : 'internal',
                ),
              ),
            ),
          );
      }
    }

    final body = message.spans.isEmpty
        ? Text(
            message.text,
            textDirection: textDirectionOf(message.dir),
            textAlign: TextAlign.start,
            style: text.body,
          )
        : Text.rich(
            TextSpan(
              children: [
                for (final span in message.spans)
                  if (span.citation case final index?)
                    WidgetSpan(
                      alignment: PlaceholderAlignment.middle,
                      child: _CitationMarker(
                        index: index,
                        onPressed: switch (_cited(index)) {
                          final cited? => () => onCitation(cited),
                          null => null,
                        },
                      ),
                    )
                  else
                    TextSpan(text: span.text),
              ],
            ),
            textDirection: textDirectionOf(message.dir),
            textAlign: TextAlign.start,
            style: text.body,
          );

    return Semantics(
      label: l10n.answer,
      container: true,
      explicitChildNodes: true,
      child: Padding(
        padding: const EdgeInsets.only(bottom: StrataSpacing.s6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Wrap(
              spacing: StrataSpacing.s2,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if (message.scopeLabel.isNotEmpty)
                  Text(
                    message.scopeLabel,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                if (message.createdLabel.isNotEmpty)
                  Text(
                    message.createdLabel,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
              ],
            ),
            const SizedBox(height: StrataSpacing.s1),
            body,
            if (message.streaming)
              Padding(
                padding: const EdgeInsets.only(top: StrataSpacing.s2),
                child: Semantics(
                  label: l10n.answering,
                  liveRegion: true,
                  child: const LinearProgressIndicator(minHeight: 2),
                ),
              ),
            if (error != null)
              Padding(
                padding: const EdgeInsets.only(top: StrataSpacing.s2),
                child: Text(
                  _stopReason(l10n, error),
                  style: text.caption.copyWith(color: colors.warningText),
                ),
              ),
            if (sources.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s3),
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.sources(count: message.sourceCount),
                    style: text.caption
                        .withWeight(FontWeight.w700)
                        .copyWith(color: colors.text2),
                  ),
                ),
              ),
              Card(
                clipBehavior: Clip.antiAlias,
                child: Column(
                  children: [
                    for (final source in sources)
                      ListTile(
                        dense: true,
                        leading: Wrap(
                          spacing: StrataSpacing.s1,
                          children: [
                            for (final index in source.indexes)
                              Text(
                                '$index',
                                style: text.monoSmall.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                          ],
                        ),
                        title: Text(source.title),
                        subtitle: Text(
                          source.path,
                          textDirection: TextDirection.ltr,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: text.monoSmall.copyWith(color: colors.text2),
                        ),
                        onTap: () => onCitation((
                          noteId: source.noteId,
                          anchor: source.anchors.isEmpty
                              ? null
                              : source.anchors.first,
                        )),
                      ),
                  ],
                ),
              ),
            ],
            if (!message.streaming) ...[
              const SizedBox(height: StrataSpacing.s2),
              Wrap(
                spacing: StrataSpacing.s2,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  if (saved == null)
                    OutlinedButton.icon(
                      onPressed: () => unawaited(save()),
                      icon: const Icon(Icons.note_add_outlined, size: 18),
                      label: Text(l10n.saveAsNote),
                    )
                  else
                    TextButton.icon(
                      onPressed: open == null ? null : () => open(saved, null),
                      icon: const Icon(Icons.check, size: 18),
                      label: Text(l10n.openSavedNote),
                    ),
                  IconButton(
                    tooltip: l10n.copyAnswer,
                    onPressed: () async {
                      await Clipboard.setData(
                        ClipboardData(text: message.text),
                      );
                      if (!context.mounted) return;
                      ScaffoldMessenger.maybeOf(context)
                        ?..hideCurrentSnackBar()
                        ..showSnackBar(SnackBar(content: Text(l10n.copied)));
                    },
                    icon: const Icon(Icons.copy_outlined, size: 18),
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// An inline citation marker ("1") that opens its source.
class _CitationMarker extends StatelessWidget {
  const new({required this.index, required this.onPressed});

  final int index;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Semantics(
      button: true,
      label: context.askL10n.citationMarker(index: index),
      excludeSemantics: true,
      child: InkWell(
        onTap: onPressed,
        customBorder: const StadiumBorder(),
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minWidth: StrataLayout.minTouchTarget,
            minHeight: StrataLayout.minTouchTarget,
          ),
          child: Center(
            widthFactor: 1,
            heightFactor: 1,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              decoration: BoxDecoration(
                color: colors.accentTint,
                borderRadius: StrataRadii.pillRadius,
              ),
              child: Text(
                '$index',
                style: context.strataText.caption
                    .withWeight(FontWeight.w700)
                    .copyWith(color: colors.accentText),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Composer extends StatelessWidget {
  const new({
    required this.controller,
    required this.scopes,
    required this.hint,
    required this.scope,
    required this.onScope,
    required this.enabled,
    required this.streaming,
    required this.onSend,
    required this.onStop,
  });

  final TextEditingController controller;
  final List<AskScope> scopes;
  final String hint;
  final int scope;
  final ValueChanged<int> onScope;
  final bool enabled;
  final bool streaming;
  final VoidCallback onSend;
  final VoidCallback onStop;

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
                if (scopes.isNotEmpty)
                  SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Semantics(
                      label: l10n.scope,
                      container: true,
                      explicitChildNodes: true,
                      child: Row(
                        children: [
                          Text(
                            l10n.scope,
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                          for (var i = 0; i < scopes.length; i++)
                            Padding(
                              padding: const EdgeInsetsDirectional.only(
                                start: StrataSpacing.s2,
                              ),
                              child: ChoiceChip(
                                label: Text(scopes[i].label),
                                selected: i == scope,
                                onSelected: (_) => onScope(i),
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),
                const SizedBox(height: StrataSpacing.s2),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    Expanded(
                      child: TextField(
                        controller: controller,
                        enabled: enabled,
                        minLines: 1,
                        maxLines: 4,
                        textInputAction: TextInputAction.send,
                        onSubmitted: (_) => onSend(),
                        decoration: InputDecoration(hintText: hint),
                      ),
                    ),
                    const SizedBox(width: StrataSpacing.s2),
                    if (streaming)
                      IconButton.filledTonal(
                        tooltip: l10n.stop,
                        onPressed: onStop,
                        icon: const Icon(Icons.stop),
                      )
                    else
                      ValueListenableBuilder(
                        valueListenable: controller,
                        builder: (context, value, _) => IconButton.filled(
                          tooltip: l10n.send,
                          onPressed: enabled && value.text.trim().isNotEmpty
                              ? onSend
                              : null,
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

/// The expanded source preview: the cited block as the core resolves it
/// (`resolve_citation`): note title, path, heading, date, the block's text
/// in its direction, tags, and "Open at block".
class _SourcePreview extends ConsumerWidget {
  const new({required this.cited, required this.onOpenNote});

  final _Cited? cited;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.askL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final shown = cited;
    final preview = shown == null
        ? null
        : ref.watch(resolveCitationProvider(shown.noteId, shown.anchor));
    final value = preview?.value;
    final open = onOpenNote;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      child: ColoredBox(
        color: colors.surface,
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(StrataSpacing.s4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
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
              else if (preview case AsyncError(:final error))
                Text(
                  l10n.errorMessage(
                    code: error is CoreFailure ? error.code : 'internal',
                  ),
                  style: text.bodySmall.copyWith(color: colors.text2),
                )
              else if (value == null)
                const AskLoading()
              else ...[
                Text(value.title, style: text.titleSmall),
                Text(
                  value.path,
                  textDirection: TextDirection.ltr,
                  style: text.monoSmall.copyWith(color: colors.text2),
                ),
                if (value.noteId == null)
                  Text(
                    l10n.noteMissing,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                if (value.heading != null || value.dateLabel != null) ...[
                  const SizedBox(height: StrataSpacing.s2),
                  Wrap(
                    spacing: StrataSpacing.s2,
                    children: [
                      if (value.heading case final heading?)
                        Text(heading, style: text.bodySmall),
                      if (value.dateLabel case final date?)
                        Text(
                          date,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
                ],
                if (value.blockText case final block?) ...[
                  const SizedBox(height: StrataSpacing.s2),
                  DecoratedBox(
                    decoration: BoxDecoration(
                      color: colors.accentTint,
                      borderRadius: StrataRadii.cardRadius,
                    ),
                    child: Padding(
                      padding: const EdgeInsets.all(StrataSpacing.s3),
                      child: Text(
                        block,
                        textDirection: textDirectionOf(value.blockDir),
                        textAlign: TextAlign.start,
                        style: text.body,
                      ),
                    ),
                  ),
                ],
                if (value.tags.isNotEmpty) ...[
                  const SizedBox(height: StrataSpacing.s2),
                  Wrap(
                    spacing: StrataSpacing.s1,
                    runSpacing: StrataSpacing.s1,
                    children: [
                      for (final tag in value.tags) Chip(label: Text(tag)),
                    ],
                  ),
                ],
                const SizedBox(height: StrataSpacing.s3),
                if (open != null && value.noteId != null)
                  FilledButton.icon(
                    onPressed: () => open(shown.noteId, shown.anchor),
                    icon: const Icon(Icons.open_in_new, size: 18),
                    label: Text(l10n.openAtBlock),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
