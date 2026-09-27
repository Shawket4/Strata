import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The autocomplete panel for the token at the caret: notes for `[[`
/// (core keyword search), people and companies for `@` (core directory,
/// matched in both scripts by the core), and the states the core cannot
/// serve yet for `#` tags and `[[Note#^` block references.
class CompletionsPanel extends StatelessWidget {
  /// Creates the panel for [controller]'s trigger.
  const new({required this.controller, super.key});

  /// The editing session.
  final NoteEditorController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final trigger = controller.trigger;
    if (trigger == null) return const SizedBox.shrink();
    final content = switch (trigger.kind) {
      TriggerKind.wikilink => _NoteSuggestions(
        controller: controller,
        query: trigger.query,
      ),
      TriggerKind.mention => _MentionSuggestions(
        controller: controller,
        query: trigger.query,
      ),
      TriggerKind.tag => _Message(l10n.suggestionsTagsUnavailable),
      TriggerKind.blockReference => _Message(l10n.suggestionsBlocksUnavailable),
    };
    return Semantics(
      container: true,
      label: l10n.suggestionsLabel,
      child: Container(
        constraints: const BoxConstraints(maxHeight: 240),
        decoration: BoxDecoration(
          color: colors.surface,
          border: Border(top: BorderSide(color: colors.border)),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: content),
            IconButton(
              tooltip: l10n.suggestionsDismiss,
              icon: const Icon(Icons.close),
              onPressed: controller.dismissTrigger,
            ),
          ],
        ),
      ),
    );
  }
}

class _Message extends StatelessWidget {
  const new(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(StrataSpacing.s4),
    child: Text(
      text,
      style: context.strataText.bodySmall.copyWith(
        color: context.strataColors.text2,
      ),
    ),
  );
}

class _NoteSuggestions extends ConsumerWidget {
  const new({required this.controller, required this.query});

  final NoteEditorController controller;
  final String query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = EditorLocalizations.of(context);
    if (query.isEmpty) return _Message(l10n.suggestionsTypeToSearch);
    final search = ref.watch(searchProvider(query, SearchMode.keyword));
    return switch (search) {
      AsyncData(:final value)
          when value.availability != Availability.available =>
        _Message(l10n.suggestionsSearchUnavailable),
      AsyncData(:final value) when value.results.isEmpty => _Message(
        l10n.suggestionsNone,
      ),
      AsyncData(:final value) => ListView(
        shrinkWrap: true,
        padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
        children: [
          _SectionLabel(l10n.suggestionsNotes),
          for (final hit in value.results)
            _SuggestionTile(
              title: hit.title,
              subtitle: hit.path,
              kind: NodeKind.note,
              onTap: () => controller.completeWikilink(hit.title),
            ),
        ],
      ),
      AsyncError() => _Message(l10n.suggestionsSearchUnavailable),
      _ => const _Loading(),
    };
  }
}

class _MentionSuggestions extends ConsumerWidget {
  const new({required this.controller, required this.query});

  final NoteEditorController controller;
  final String query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = EditorLocalizations.of(context);
    final people = ref.watch(directoryProvider(DirectoryTab.people, query));
    final companies = ref.watch(
      directoryProvider(DirectoryTab.companies, query),
    );
    if (people is AsyncLoading || companies is AsyncLoading) {
      if (!people.hasValue && !companies.hasValue) return const _Loading();
    }
    final personItems = people.value?.items ?? const <DirectoryItem>[];
    final companyItems = companies.value?.items ?? const <DirectoryItem>[];
    if (personItems.isEmpty && companyItems.isEmpty) {
      return _Message(l10n.suggestionsNone);
    }
    return ListView(
      shrinkWrap: true,
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
      children: [
        if (personItems.isNotEmpty) _SectionLabel(l10n.suggestionsPeople),
        for (final item in personItems)
          _SuggestionTile(
            title: item.title,
            subtitle: item.subtitle,
            kind: NodeKind.person,
            onTap: () => unawaited(
              controller.completeMention(item, relationKey: 'people'),
            ),
          ),
        if (companyItems.isNotEmpty) _SectionLabel(l10n.suggestionsCompanies),
        for (final item in companyItems)
          _SuggestionTile(
            title: item.title,
            subtitle: item.subtitle,
            kind: NodeKind.company,
            onTap: () => unawaited(
              controller.completeMention(item, relationKey: 'companies'),
            ),
          ),
      ],
    );
  }
}

class _Loading extends StatelessWidget {
  const new();

  // A static label (no spinner): results usually arrive within a frame.
  @override
  Widget build(BuildContext context) =>
      _Message(EditorLocalizations.of(context).suggestionsLoading);
}

class _SectionLabel extends StatelessWidget {
  const new(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsetsDirectional.fromSTEB(
      StrataSpacing.s4,
      StrataSpacing.s2,
      StrataSpacing.s4,
      StrataSpacing.s1,
    ),
    child: Text(
      text,
      style: context.strataText.caption
          .copyWith(color: context.strataColors.text2)
          .copyWith(fontWeight: FontWeight.w600),
    ),
  );
}

class _SuggestionTile extends StatelessWidget {
  const new({
    required this.title,
    required this.kind,
    required this.onTap,
    this.subtitle,
  });

  final String title;
  final String? subtitle;
  final NodeKind kind;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final sub = subtitle;
    return MergeSemantics(
      child: InkWell(
        onTap: onTap,
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minHeight: StrataLayout.minTouchTarget,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s4,
              vertical: StrataSpacing.s1,
            ),
            child: Row(
              children: [
                NodeKindGlyph(kind: kind, decorative: true),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: text.bodySmall.copyWith(
                          color: colors.text,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      if (sub != null)
                        Text(
                          sub,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: text.caption.copyWith(color: colors.text2),
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
