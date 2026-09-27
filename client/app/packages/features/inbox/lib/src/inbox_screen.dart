import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_inbox/src/capture_card.dart';
import 'package:strata_inbox/src/l10n.dart';
import 'package:strata_inbox/src/proposals.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// The key of a rendered inbox row: a capture or a standalone suggestion.
typedef _RowKey = ({bool capture, String id});

/// The Inbox: captures with the AI's proposal (Accept / Edit / Reject),
/// entity link-or-create cards, custody items (applied automatically with
/// Undo, or suggestions), duplicate-flagged items and other suggestions.
///
/// * compact: one list of cards;
/// * medium: list + detail pane;
/// * expanded: list with bulk selection and bulk bar + detail with the
///   capture beside its proposal; keyboard J/K move, X select, A accept,
///   R reject, E edit.
class InboxScreen extends HookConsumerWidget {
  /// Creates the Inbox.
  const new({
    super.key,
    this.onOpenNote,
    this.onOpenEntity,
    this.initialNoteId,
  });

  /// The icon that represents this feature.
  static const IconData icon = Icons.move_to_inbox_outlined;

  /// Opens a note by id (a capture, a relation target, an existing item).
  final ValueChanged<String>? onOpenNote;

  /// Opens a person, company, document or place by id.
  final ValueChanged<String>? onOpenEntity;

  /// The capture selected when the screen opens (deep link).
  final String? initialNoteId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final value = ref.watch(inboxProvider);
    final initial = initialNoteId;
    final selected = useState<_RowKey?>(
      initial == null ? null : (capture: true, id: initial),
    );
    final checked = useState<Set<String>>(const {});
    final sizeClass = SizeClass.of(context);
    return InboxL10nScope(
      child: Builder(
        builder: (context) => CoreAsyncBody(
          value: value,
          errorTitle: context.inboxL10n.inboxLoadError,
          data: (view) {
            final l10n = context.inboxL10n;
            if (view.captures.isEmpty && view.suggestions.isEmpty) {
              return StrataEmptyState(
                icon: InboxScreen.icon,
                title: l10n.inboxEmptyTitle,
                message: l10n.inboxEmptyMessage,
              );
            }
            if (sizeClass == SizeClass.compact) {
              return ListView(
                padding: const EdgeInsets.all(StrataSpacing.s4),
                children: [
                  for (final suggestion in view.suggestions) ...[
                    SuggestionCard(
                      suggestion: suggestion,
                      onOpenNote: onOpenNote,
                      onOpenEntity: onOpenEntity,
                    ),
                    const SizedBox(height: StrataSpacing.s3),
                  ],
                  for (final item in view.captures) ...[
                    CaptureCard(
                      item: item,
                      onOpenNote: onOpenNote,
                      onOpenEntity: onOpenEntity,
                    ),
                    const SizedBox(height: StrataSpacing.s3),
                  ],
                ],
              );
            }
            final rows = <_RowKey>[
              for (final s in view.suggestions) (capture: false, id: s.id),
              for (final c in view.captures) (capture: true, id: c.noteId),
            ];
            final chosen = selected.value;
            final current = chosen != null && rows.contains(chosen)
                ? chosen
                : view.captures.isNotEmpty
                ? (capture: true, id: view.captures.first.noteId)
                : rows.first;
            final bulk = sizeClass == SizeClass.expanded;
            final list = _InboxList(
              view: view,
              selected: current,
              onSelect: (key) => selected.value = key,
              bulk: bulk,
              checked: checked.value,
              onChecked: (value) => checked.value = value,
            );
            final detail = _InboxDetail(
              view: view,
              selected: current,
              twoColumns: bulk,
              onOpenNote: onOpenNote,
              onOpenEntity: onOpenEntity,
            );
            return _InboxKeyboard(
              view: view,
              rows: rows,
              current: current,
              onSelect: (key) => selected.value = key,
              checked: checked.value,
              onChecked: (value) => checked.value = value,
              onOpenNote: onOpenNote,
              child: bulk
                  ? Row(
                      children: [
                        SizedBox(width: 400, child: list),
                        VerticalDivider(
                          width: 1,
                          color: context.strataColors.border,
                        ),
                        Expanded(child: detail),
                      ],
                    )
                  : StrataPanes(list: list, detail: detail),
            );
          },
        ),
      ),
    );
  }
}

InboxItem? _capture(InboxView view, String noteId) {
  for (final item in view.captures) {
    if (item.noteId == noteId) return item;
  }
  return null;
}

SuggestionItem? _suggestion(InboxView view, String id) {
  for (final item in view.suggestions) {
    if (item.id == id) return item;
  }
  return null;
}

class _InboxKeyboard extends ConsumerWidget {
  const new({
    required this.view,
    required this.rows,
    required this.current,
    required this.onSelect,
    required this.checked,
    required this.onChecked,
    required this.onOpenNote,
    required this.child,
  });

  final InboxView view;
  final List<_RowKey> rows;
  final _RowKey current;
  final ValueChanged<_RowKey> onSelect;
  final Set<String> checked;
  final ValueChanged<Set<String>> onChecked;
  final ValueChanged<String>? onOpenNote;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final index = rows.indexOf(current);
    final api = ref.read(coreApiProvider);
    void move(int delta) =>
        onSelect(rows[(index + delta).clamp(0, rows.length - 1)]);

    void act({required bool accept}) {
      if (current.capture) {
        final item = _capture(view, current.id);
        if (item == null) return;
        unawaited(
          accept
              ? acceptCapture(context, api, item)
              : rejectCapture(context, api, item),
        );
        return;
      }
      unawaited(
        forwardIntent(
          context,
          accept
              ? api.acceptSuggestion(id: current.id)
              : api.rejectSuggestion(id: current.id),
        ),
      );
    }

    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyJ): () => move(1),
        const SingleActivator(LogicalKeyboardKey.keyK): () => move(-1),
        const SingleActivator(LogicalKeyboardKey.keyX): () {
          if (!current.capture) return;
          onChecked(
            checked.contains(current.id)
                ? {...checked}.difference({current.id})
                : {...checked, current.id},
          );
        },
        const SingleActivator(LogicalKeyboardKey.keyA): () => act(accept: true),
        const SingleActivator(LogicalKeyboardKey.keyR): () =>
            act(accept: false),
        const SingleActivator(LogicalKeyboardKey.keyE): () {
          final open = onOpenNote;
          if (open == null) return;
          final noteId = current.capture
              ? current.id
              : _suggestion(view, current.id)?.noteId;
          if (noteId != null) open(noteId);
        },
      },
      child: Focus(autofocus: true, includeSemantics: false, child: child),
    );
  }
}

class _InboxList extends ConsumerWidget {
  const new({
    required this.view,
    required this.selected,
    required this.onSelect,
    required this.bulk,
    required this.checked,
    required this.onChecked,
  });

  final InboxView view;
  final _RowKey selected;
  final ValueChanged<_RowKey> onSelect;
  final bool bulk;
  final Set<String> checked;
  final ValueChanged<Set<String>> onChecked;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final api = ref.read(coreApiProvider);
    final checkedCaptures = [
      for (final item in view.captures)
        if (checked.contains(item.noteId)) item,
    ];
    Future<void> bulkAct({required bool accept}) async {
      for (final item in checkedCaptures) {
        await (accept
            ? acceptCapture(context, api, item)
            : rejectCapture(context, api, item));
        if (!context.mounted) return;
      }
      onChecked(const {});
    }

    return ListView(
      children: [
        Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s4,
            StrataSpacing.s4,
            StrataSpacing.s4,
            StrataSpacing.s2,
          ),
          child: Semantics(
            header: true,
            container: true,
            child: Text(l10n.inboxTitle, style: text.title),
          ),
        ),
        if (bulk)
          Semantics(
            container: true,
            label: l10n.inboxBulkActions,
            child: Container(
              margin: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
              padding: const EdgeInsetsDirectional.only(end: StrataSpacing.s2),
              decoration: BoxDecoration(
                color: colors.surface2,
                borderRadius: StrataRadii.cardRadius,
              ),
              child: Wrap(
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: StrataSpacing.s1,
                children: [
                  Checkbox(
                    tristate: true,
                    value: checkedCaptures.isEmpty
                        ? false
                        : checkedCaptures.length == view.captures.length
                        ? true
                        : null,
                    semanticLabel: l10n.inboxSelectAll,
                    onChanged: (_) => onChecked(
                      checkedCaptures.length == view.captures.length
                          ? const {}
                          : {for (final c in view.captures) c.noteId},
                    ),
                  ),
                  Text(
                    l10n.inboxSelectedCount(count: checkedCaptures.length),
                    style: text.bodySmall.withWeight(FontWeight.w600),
                  ),
                  TextButton(
                    onPressed: checkedCaptures.isEmpty
                        ? null
                        : () => unawaited(bulkAct(accept: true)),
                    child: Text(
                      l10n.inboxAccept,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  TextButton(
                    onPressed: checkedCaptures.isEmpty
                        ? null
                        : () => unawaited(bulkAct(accept: false)),
                    child: Text(
                      l10n.inboxReject,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  IconButton(
                    tooltip: l10n.inboxClearSelection,
                    onPressed: checkedCaptures.isEmpty
                        ? null
                        : () => onChecked(const {}),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
          ),
        if (view.suggestions.isNotEmpty) ...[
          StrataSectionHeader(
            title: l10n.inboxSuggestionsHeader,
            count: view.suggestions.length,
          ),
          for (final suggestion in view.suggestions)
            _SuggestionRow(
              suggestion: suggestion,
              selected: !selected.capture && selected.id == suggestion.id,
              onTap: () => onSelect((capture: false, id: suggestion.id)),
            ),
        ],
        if (view.captures.isNotEmpty) ...[
          StrataSectionHeader(
            title: l10n.inboxCapturesHeader,
            count: view.captures.length,
          ),
          for (final item in view.captures)
            _CaptureRow(
              item: item,
              selected: selected.capture && selected.id == item.noteId,
              onTap: () => onSelect((capture: true, id: item.noteId)),
              checked: bulk ? checked.contains(item.noteId) : null,
              onChecked: (value) => onChecked(
                value
                    ? {...checked, item.noteId}
                    : {...checked}.difference({item.noteId}),
              ),
            ),
        ],
        if (bulk)
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Semantics(
              container: true,
              label: l10n.inboxShortcutsLabel,
              child: Wrap(
                spacing: StrataSpacing.s3,
                runSpacing: StrataSpacing.s1,
                children: [
                  _Hint(keys: const ['J', 'K'], label: l10n.inboxKeyMove),
                  _Hint(keys: const ['X'], label: l10n.inboxKeySelect),
                  _Hint(keys: const ['A'], label: l10n.inboxKeyAccept),
                  _Hint(keys: const ['R'], label: l10n.inboxKeyReject),
                  _Hint(keys: const ['E'], label: l10n.inboxKeyEdit),
                ],
              ),
            ),
          ),
      ],
    );
  }
}

class _Hint extends StatelessWidget {
  const new({required this.keys, required this.label});

  final List<String> keys;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        KeyboardHintChip(keys: keys),
        const SizedBox(width: StrataSpacing.s1),
        ExcludeSemantics(
          child: Text(
            label,
            style: context.strataText.caption.copyWith(
              color: context.strataColors.text2,
            ),
          ),
        ),
      ],
    );
  }
}

class _SuggestionRow extends StatelessWidget {
  const new({
    required this.suggestion,
    required this.selected,
    required this.onTap,
  });

  final SuggestionItem suggestion;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final detail = suggestion.detail;
    final summary = switch (detail.kind) {
      SuggestionKind.entityLinkOrCreate => l10n.inboxWhoIs(
        mention: detail.mention,
      ),
      SuggestionKind.custody || SuggestionKind.task => detail.line,
      SuggestionKind.duplicate => l10n.inboxPossibleDuplicate,
      SuggestionKind.relation => l10n.inboxRelationSummary(
        type: context.l10n.relationTypeLabel(
          CoreLabels.relationType(detail.relType),
        ),
        target: detail.target?.title ?? '',
      ),
      SuggestionKind.filing => detail.title,
      SuggestionKind.unsupported => detail.serverKind,
    };
    final badge = switch (detail.kind) {
      SuggestionKind.entityLinkOrCreate => l10n.inboxNeedsYou,
      SuggestionKind.custody when suggestion.status == 'accepted' =>
        l10n.inboxAppliedAutomatically,
      SuggestionKind.custody => l10n.inboxCustodySuggestion,
      SuggestionKind.duplicate => l10n.inboxPossibleDuplicate,
      SuggestionKind.relation => l10n.inboxRelationSuggestion,
      SuggestionKind.task => l10n.inboxTaskSuggestion,
      SuggestionKind.filing => l10n.inboxFilingProposal,
      SuggestionKind.unsupported => l10n.inboxUnsupportedBadge,
    };
    return Material(
      color: selected ? colors.accentTint : Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: StrataSpacing.s4,
            vertical: StrataSpacing.s3,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                badge,
                style: text.caption
                    .withWeight(FontWeight.w700)
                    .copyWith(color: colors.warningText),
              ),
              Text(
                summary,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: text.bodySmall.withWeight(FontWeight.w500),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CaptureRow extends StatelessWidget {
  const new({
    required this.item,
    required this.selected,
    required this.onTap,
    required this.checked,
    required this.onChecked,
  });

  final InboxItem item;
  final bool selected;
  final VoidCallback onTap;
  final bool? checked;
  final ValueChanged<bool> onChecked;

  @override
  Widget build(BuildContext context) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final isChecked = checked;
    return Material(
      color: selected ? colors.accentTint : Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s2,
            StrataSpacing.s1,
            StrataSpacing.s4,
            StrataSpacing.s1,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (isChecked != null)
                Checkbox(
                  value: isChecked,
                  semanticLabel: l10n.inboxSelectCapture,
                  onChanged: (value) => onChecked(value ?? false),
                )
              else
                const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    vertical: StrataSpacing.s2,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      CaptureText(text: item.text, maxLines: 2),
                      const SizedBox(height: 2),
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              item.suggestions.isEmpty
                                  ? l10n.inboxNoProposalYet
                                  : l10n.inboxProposalCount(
                                      count: item.suggestions.length,
                                    ),
                              style: text.caption.copyWith(color: colors.text2),
                            ),
                          ),
                          if (item.pendingSync) ...[
                            const SizedBox(width: StrataSpacing.s1),
                            const NotSyncedMarker(),
                          ],
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _InboxDetail extends StatelessWidget {
  const new({
    required this.view,
    required this.selected,
    required this.twoColumns,
    required this.onOpenNote,
    required this.onOpenEntity,
  });

  final InboxView view;
  final _RowKey selected;
  final bool twoColumns;
  final ValueChanged<String>? onOpenNote;
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context) {
    final l10n = context.inboxL10n;
    if (!selected.capture) {
      final suggestion = _suggestion(view, selected.id);
      if (suggestion == null) return const SizedBox.shrink();
      return ListView(
        padding: const EdgeInsets.all(StrataSpacing.s6),
        children: [
          SuggestionCard(
            suggestion: suggestion,
            onOpenNote: onOpenNote,
            onOpenEntity: onOpenEntity,
          ),
        ],
      );
    }
    final item = _capture(view, selected.id);
    if (item == null) return const SizedBox.shrink();
    final capture = _CapturePanel(item: item);
    final proposal = Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        child: CaptureCard(
          item: item,
          density: ProposalDensity.detail,
          showText: false,
          onOpenNote: onOpenNote,
          onOpenEntity: onOpenEntity,
        ),
      ),
    );
    final alsoNeedsYou = [
      if (view.suggestions.isNotEmpty) ...[
        StrataSectionHeader(
          title: l10n.inboxAlsoNeedsYou,
          padding: const EdgeInsets.only(
            top: StrataSpacing.s4,
            bottom: StrataSpacing.s2,
          ),
        ),
        for (final suggestion in view.suggestions) ...[
          SuggestionCard(
            suggestion: suggestion,
            onOpenNote: onOpenNote,
            onOpenEntity: onOpenEntity,
          ),
          const SizedBox(height: StrataSpacing.s3),
        ],
      ],
    ];
    if (!twoColumns) {
      return ListView(
        padding: const EdgeInsets.all(StrataSpacing.s5),
        children: [
          capture,
          const SizedBox(height: StrataSpacing.s4),
          proposal,
        ],
      );
    }
    return SingleChildScrollView(
      padding: const EdgeInsets.all(StrataSpacing.s6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [capture, ...alsoNeedsYou],
            ),
          ),
          const SizedBox(width: StrataSpacing.s5),
          Expanded(child: proposal),
        ],
      ),
    );
  }
}

class _CapturePanel extends StatelessWidget {
  const new({required this.item});

  final InboxItem item;

  @override
  Widget build(BuildContext context) {
    final l10n = context.inboxL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final meta = text.monoSmall.copyWith(color: colors.text2);
    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            StrataSectionHeader(
              title: l10n.inboxCaptureHeader,
              padding: EdgeInsets.zero,
              trailing: item.pendingSync ? const NotSyncedMarker() : null,
            ),
            const SizedBox(height: StrataSpacing.s2),
            Text(
              item.text,
              textAlign: TextAlign.start,
              style: text.titleSmall.withWeight(FontWeight.w500),
            ),
            const SizedBox(height: StrataSpacing.s3),
            Text(item.title, style: meta, textDirection: TextDirection.ltr),
            Text(item.noteId, style: meta, textDirection: TextDirection.ltr),
          ],
        ),
      ),
    );
  }
}
