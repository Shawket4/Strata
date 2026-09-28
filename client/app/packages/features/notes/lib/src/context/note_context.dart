import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/strata_maps.dart' show MiniGraph;
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_notes/src/notes_scope.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Backlinks grouped by relation type as the core groups, labels and
/// orders them: each source with its linking sentence (in its own
/// direction) and, for AI relations, the confidence. Each source opens on
/// tap.
class BacklinksSection extends StatelessWidget {
  /// Creates the section.
  const new({required this.groups, super.key, this.onOpenNote});

  /// The groups.
  final List<BacklinkGroup> groups;

  /// Opens a note.
  final ValueChanged<String>? onOpenNote;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    if (groups.isEmpty) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
        child: Text(
          l10n.noBacklinks,
          style: text.bodySmall.copyWith(color: colors.text2),
        ),
      );
    }
    final open = onOpenNote;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.backlinksByType,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final group in groups) ...[
            _GroupHeader(group: group),
            for (final item in group.items)
              Padding(
                padding: const EdgeInsets.only(bottom: StrataSpacing.s1 + 2),
                child: MergeSemantics(
                  child: Material(
                    color: colors.surface,
                    shape: RoundedRectangleBorder(
                      borderRadius: StrataRadii.inputRadius,
                      side: BorderSide(color: colors.border),
                    ),
                    child: InkWell(
                      borderRadius: StrataRadii.inputRadius,
                      onTap: open == null ? null : () => open(item.noteId),
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(
                          minHeight: StrataLayout.minTouchTarget,
                        ),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: StrataSpacing.s3,
                            vertical: StrataSpacing.s2,
                          ),
                          child: _BacklinkBody(item: item),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            const SizedBox(height: StrataSpacing.s2),
          ],
        ],
      ),
    );
  }
}

class _BacklinkBody extends StatelessWidget {
  const new({required this.item});

  final BacklinkItem item;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final snippet = item.snippet;
    final confidence = item.confidence;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                item.title,
                textDirection: textDirectionOf(item.titleDir),
                textAlign: TextAlign.start,
                style: text.bodySmall.copyWith(
                  color: colors.text,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (item.by == 'ai' && confidence != null) ...[
              const SizedBox(width: StrataSpacing.s2),
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: StrataSpacing.s2 - 2,
                ),
                decoration: BoxDecoration(
                  color: colors.infoTint,
                  borderRadius: StrataRadii.pillRadius,
                ),
                child: Text(
                  l10n.aiConfidenceTag(value: confidence.toStringAsFixed(2)),
                  style: text.caption
                      .withWeight(FontWeight.w600)
                      .copyWith(color: colors.infoText),
                ),
              ),
            ],
          ],
        ),
        if (snippet != null && snippet.isNotEmpty)
          Text(
            snippet,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            textDirection: textDirectionOf(item.snippetDir),
            textAlign: TextAlign.start,
            style: text.caption.copyWith(color: colors.text2),
          ),
      ],
    );
  }
}

class _GroupHeader extends StatelessWidget {
  const new({required this.group});

  final BacklinkGroup group;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s1 + 2),
      child: Semantics(
        header: true,
        child: Row(
          children: [
            RelationLineSample(
              type: relationTypeOf(group.kind),
              mentionOf: mentionKindOf(group.kind),
            ),
            const SizedBox(width: StrataSpacing.s2),
            Flexible(
              child: Text(
                group.label,
                style: text.caption
                    .copyWith(color: colors.text)
                    .copyWith(fontWeight: FontWeight.w600),
              ),
            ),
            const SizedBox(width: StrataSpacing.s2),
            Text(
              '${group.items.length}',
              style: text.caption.copyWith(color: colors.text2),
            ),
          ],
        ),
      ),
    );
  }
}

/// The local mini graph of the note (the maps feature's [MiniGraph] over
/// the core's local graph stream), with "Open map" for the mind map.
class LocalGraphSlot extends StatelessWidget {
  /// Creates the slot for [noteId].
  const new({
    required this.noteId,
    super.key,
    this.onOpenMap,
    this.height = 180,
  });

  /// The note in the centre.
  final String noteId;

  /// Opens the local mind map.
  final VoidCallback? onOpenMap;

  /// Height of the graph canvas.
  final double height;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.localGraphTitle,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Semantics(
                header: true,
                child: Text(
                  l10n.localGraphTitle,
                  style: text.caption
                      .copyWith(color: colors.text2)
                      .copyWith(fontWeight: FontWeight.w600),
                ),
              ),
              TextButton(
                onPressed: onOpenMap,
                style: TextButton.styleFrom(
                  foregroundColor: colors.accentText,
                  minimumSize: const Size(48, StrataLayout.minTouchTarget),
                ),
                child: Text(l10n.openMap),
              ),
            ],
          ),
          // The mini graph brings its own card; its header is ours.
          MiniGraph(
            noteId,
            height: height,
            showHeader: false,
            padding: EdgeInsets.zero,
          ),
        ],
      ),
    );
  }
}

/// Version history with revert (online): the core's entries
/// (`refresh_history` fills them when the section opens), each with its
/// version, message, author and time; "Changes" shows the revision against
/// the current text (`note_revision_diff`), "Revert" asks first and calls
/// `revert_note`. Offline or unavailable history renders its message.
class HistorySection extends ConsumerStatefulWidget {
  /// Creates the section for [note].
  const new({required this.note, super.key});

  /// The note.
  final NoteView note;

  @override
  ConsumerState<HistorySection> createState() => _HistorySectionState();
}

class _HistorySectionState extends ConsumerState<HistorySection> {
  @override
  void initState() {
    super.initState();
    _refresh();
  }

  @override
  void didUpdateWidget(HistorySection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.note.id != widget.note.id ||
        oldWidget.note.history != widget.note.history) {
      _refresh();
    }
  }

  void _refresh() {
    if (widget.note.history != Availability.available) return;
    unawaited(
      ref
          .read(coreApiProvider)
          .refreshHistory(noteId: widget.note.id)
          .catchError((Object _) {}),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final note = widget.note;
    final entries = note.historyEntries;
    final Widget body;
    if (note.history != Availability.available) {
      final (icon, message) = switch (note.history) {
        Availability.offline => (Icons.cloud_off_outlined, l10n.historyOffline),
        Availability.notAllowed => (Icons.block, l10n.historyNotAllowed),
        _ => (Icons.hourglass_empty, l10n.historyNotYetAvailable),
      };
      body = Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 18, color: colors.text2),
          const SizedBox(width: StrataSpacing.s2),
          Expanded(
            child: Text(
              message,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          ),
        ],
      );
    } else if (entries.isEmpty) {
      body = Text(
        l10n.historyEmpty,
        style: text.bodySmall.copyWith(color: colors.text2),
      );
    } else {
      body = Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final entry in entries) _HistoryRow(note: note, entry: entry),
        ],
      );
    }
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.historyTitle,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.historyTitle,
                    style: text.caption
                        .copyWith(color: colors.text2)
                        .copyWith(fontWeight: FontWeight.w600),
                  ),
                ),
              ),
              if (entries.isNotEmpty)
                Flexible(
                  child: Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: Text(
                      l10n.historyAll(count: entries.length),
                      textAlign: TextAlign.end,
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s2),
          body,
        ],
      ),
    );
  }
}

String _author(NotesLocalizations l10n, String author) => switch (author) {
  'ai' => l10n.authorAi,
  'system' => l10n.authorSystem,
  _ => l10n.authorUser,
};

class _HistoryRow extends ConsumerWidget {
  const new({required this.note, required this.entry});

  final NoteView note;
  final HistoryEntry entry;

  Future<void> _diff(BuildContext context, WidgetRef ref) async {
    final l10n = NotesLocalizations.of(context);
    final messenger = ScaffoldMessenger.maybeOf(context);
    try {
      final diff = await ref
          .read(coreApiProvider)
          .noteRevisionDiff(noteId: note.id, commit: entry.commit);
      if (!context.mounted) return;
      await showDialog<void>(
        context: context,
        builder: (dialog) => NotesLocalizationsScope(
          child: NoteDiffDialog(version: entry.versionLabel, diff: diff),
        ),
      );
    } on Object catch (error) {
      messenger?.showSnackBar(
        SnackBar(content: Text(noteFailure(l10n, error))),
      );
    }
  }

  Future<void> _revert(BuildContext context, WidgetRef ref) async {
    final l10n = NotesLocalizations.of(context);
    final messenger = ScaffoldMessenger.maybeOf(context);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialog) => AlertDialog(
        title: Text(l10n.revertTitle(version: entry.versionLabel)),
        content: Text(l10n.revertBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialog).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialog).pop(true),
            child: Text(l10n.revert),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await ref
          .read(coreApiProvider)
          .revertNote(noteId: note.id, commit: entry.commit);
    } on Object catch (error) {
      messenger?.showSnackBar(
        SnackBar(content: Text(noteFailure(l10n, error))),
      );
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          MergeSemantics(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(
                  width: 32,
                  child: Text(
                    entry.versionLabel,
                    style: text.monoSmall.copyWith(color: colors.text2),
                  ),
                ),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        entry.message,
                        style: text.bodySmall.copyWith(color: colors.text),
                      ),
                      Text(
                        l10n.historyWho(
                          author: _author(l10n, entry.author),
                          when: entry.atLabel,
                        ),
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Wrap(
            alignment: WrapAlignment.end,
            children: [
              TextButton(
                onPressed: () => unawaited(_diff(context, ref)),
                child: Text(l10n.viewChanges),
              ),
              if (entry.canRevert)
                TextButton(
                  onPressed: () => unawaited(_revert(context, ref)),
                  child: Text(l10n.revert),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// The message of a failed intent.
String noteFailure(NotesLocalizations l10n, Object error) => switch (error) {
  CoreFailure(:final code) => l10n.noteFailed(code: code),
  _ => l10n.noteFailed(code: 'internal'),
};

/// A revision compared with the note's current text, line by line as the
/// core diffed it.
class NoteDiffDialog extends StatelessWidget {
  /// Creates the dialog.
  const new({required this.version, required this.diff, super.key});

  /// The revision's label ("v6").
  final String version;

  /// The core's diff.
  final NoteDiffView diff;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return AlertDialog(
      title: Text(l10n.diffTitle(version: version)),
      scrollable: true,
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(diff.summary, style: text.caption.copyWith(color: colors.text2)),
          const SizedBox(height: StrataSpacing.s2),
          for (final line in diff.lines)
            ColoredBox(
              color: switch (line.kind) {
                DiffLineKind.added => colors.successTint,
                DiffLineKind.removed => colors.dangerTint,
                DiffLineKind.same => Colors.transparent,
              },
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: StrataSpacing.s2,
                  vertical: 1,
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 16,
                      child: Text(switch (line.kind) {
                        DiffLineKind.added => '+',
                        DiffLineKind.removed => '−',
                        DiffLineKind.same => '',
                      }, style: text.monoSmall.copyWith(color: colors.text2)),
                    ),
                    Expanded(
                      child: Text(
                        line.text,
                        textDirection: textDirectionOf(line.dir),
                        textAlign: TextAlign.start,
                        style: text.monoSmall.copyWith(color: colors.text),
                      ),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    );
  }
}

/// Which context tab is shown.
enum ContextTab {
  /// Backlinks (with the graph slot and history below, as designed).
  backlinks,

  /// The local graph slot.
  graph,

  /// History.
  history,
}

/// The note's context: Backlinks / Graph / History tabs. Used as the
/// expanded context panel and the medium drawer (with a title and close
/// button when [onClose] is set).
class NoteContextPanel extends StatefulWidget {
  /// Creates the panel for [note].
  const new({
    required this.note,
    super.key,
    this.onOpenNote,
    this.onOpenMap,
    this.onClose,
  });

  /// The note.
  final NoteView note;

  /// Opens a note.
  final ValueChanged<String>? onOpenNote;

  /// Opens the local mind map.
  final VoidCallback? onOpenMap;

  /// Closes the drawer (medium).
  final VoidCallback? onClose;

  @override
  State<NoteContextPanel> createState() => _NoteContextPanelState();
}

class _NoteContextPanelState extends State<NoteContextPanel> {
  ContextTab _tab = ContextTab.backlinks;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final note = widget.note;
    final close = widget.onClose;
    final content = switch (_tab) {
      ContextTab.backlinks => [
        BacklinksSection(groups: note.backlinks, onOpenNote: widget.onOpenNote),
        const SizedBox(height: StrataSpacing.s3),
        Divider(color: colors.border, height: 1),
        const SizedBox(height: StrataSpacing.s3),
        LocalGraphSlot(noteId: note.id, onOpenMap: widget.onOpenMap),
        const SizedBox(height: StrataSpacing.s3),
        Divider(color: colors.border, height: 1),
        const SizedBox(height: StrataSpacing.s3),
        HistorySection(note: note),
      ],
      ContextTab.graph => [
        LocalGraphSlot(
          noteId: note.id,
          onOpenMap: widget.onOpenMap,
          height: 280,
        ),
      ],
      ContextTab.history => [HistorySection(note: note)],
    };
    return Material(
      color: close == null ? colors.background : colors.surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (close != null)
            Padding(
              padding: const EdgeInsetsDirectional.fromSTEB(
                StrataSpacing.s5,
                StrataSpacing.s2,
                StrataSpacing.s2,
                0,
              ),
              child: Row(
                children: [
                  Expanded(
                    child: Semantics(
                      header: true,
                      child: Text(
                        l10n.contextTitle,
                        style: text.titleSmall.copyWith(color: colors.text),
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: context.l10n.actionClose,
                    icon: const Icon(Icons.close),
                    onPressed: close,
                  ),
                ],
              ),
            ),
          DecoratedBox(
            decoration: BoxDecoration(
              border: Border(bottom: BorderSide(color: colors.border)),
            ),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
              child: Row(
                children: [
                  for (final tab in ContextTab.values)
                    _TabButton(
                      label: switch (tab) {
                        ContextTab.backlinks => l10n.backlinksTab(
                          count: note.backlinkCount,
                        ),
                        ContextTab.graph => l10n.tabGraph,
                        ContextTab.history => l10n.tabHistory,
                      },
                      selected: _tab == tab,
                      onTap: () => setState(() => _tab = tab),
                    ),
                ],
              ),
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              children: content,
            ),
          ),
        ],
      ),
    );
  }
}

class _TabButton extends StatelessWidget {
  const new({required this.label, required this.selected, required this.onTap});

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      selected: selected,
      button: true,
      child: InkWell(
        onTap: onTap,
        child: Container(
          constraints: const BoxConstraints(
            minHeight: StrataLayout.minTouchTarget,
          ),
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
          decoration: BoxDecoration(
            border: Border(
              bottom: BorderSide(
                color: selected ? colors.accent : Colors.transparent,
                width: 2,
              ),
            ),
          ),
          alignment: Alignment.center,
          child: Text(
            label,
            style: text.bodySmall.copyWith(
              color: selected ? colors.text : colors.text2,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}
