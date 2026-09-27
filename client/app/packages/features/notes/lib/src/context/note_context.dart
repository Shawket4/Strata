import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_notes/src/generated/notes_localizations.dart';
import 'package:strata_notes/src/notes_scope.dart';
import 'package:strata_state/strata_state.dart'
    show Availability, BacklinkGroup, NoteView;
import 'package:strata_ui/strata_ui.dart';

/// Backlinks grouped by relation type (`link` = plain body links), as the
/// core groups and orders them. Each source opens on tap.
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
                child: Material(
                  color: colors.surface,
                  shape: RoundedRectangleBorder(
                    borderRadius: StrataRadii.inputRadius,
                    side: BorderSide(color: colors.border),
                  ),
                  child: InkWell(
                    borderRadius: StrataRadii.inputRadius,
                    onTap: onOpenNote == null
                        ? null
                        : () => onOpenNote!(item.noteId),
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(
                        minHeight: StrataLayout.minTouchTarget,
                      ),
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: StrataSpacing.s3,
                          vertical: StrataSpacing.s2,
                        ),
                        child: Align(
                          alignment: AlignmentDirectional.centerStart,
                          child: Text(
                            item.title,
                            textAlign: TextAlign.start,
                            style: text.bodySmall.copyWith(
                              color: colors.text,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
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

class _GroupHeader extends StatelessWidget {
  const new({required this.group});

  final BacklinkGroup group;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final type = relationTypeOf(group.kind);
    final label = group.kind == 'link'
        ? l10n.backlinkKindLink
        : isNamedRelation(group.kind)
        ? context.l10n.relationTypeLabel(type)
        : group.kind;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s1 + 2),
      child: Semantics(
        header: true,
        child: Row(
          children: [
            RelationLineSample(type: type, mentionOf: mentionKindOf(group.kind)),
            const SizedBox(width: StrataSpacing.s2),
            Flexible(
              child: Text(
                label,
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

/// The local mini-graph slot. The maps feature exports no embeddable mini
/// graph yet, so the slot shows a placeholder with "Open map"
/// (docs/CORE_GAPS.md, wiring note).
class LocalGraphSlot extends StatelessWidget {
  /// Creates the slot.
  const new({super.key, this.onOpenMap, this.height = 160});

  /// Opens the local mind map.
  final VoidCallback? onOpenMap;

  /// Height of the placeholder canvas.
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
          Row(
            children: [
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.localGraphTitle,
                    style: text.caption
                        .copyWith(color: colors.text2)
                        .copyWith(fontWeight: FontWeight.w600),
                  ),
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
          Container(
            constraints: BoxConstraints(minHeight: height),
            padding: const EdgeInsets.all(StrataSpacing.s4),
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border.all(color: colors.border),
              borderRadius: StrataRadii.cardRadius,
            ),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                const Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    NodeKindGlyph(kind: NodeKind.concept, decorative: true),
                    SizedBox(width: StrataSpacing.s3),
                    NodeKindGlyph(
                      kind: NodeKind.note,
                      size: 22,
                      selected: true,
                      decorative: true,
                    ),
                    SizedBox(width: StrataSpacing.s3),
                    NodeKindGlyph(kind: NodeKind.person, decorative: true),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s3),
                Text(
                  l10n.localGraphPlaceholder,
                  textAlign: TextAlign.center,
                  style: text.caption.copyWith(color: colors.text2),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Version history with revert. The core streams only whether history can
/// be used (`NoteView.history`); the entries and the revert intent are not
/// in the core yet (docs/CORE_GAPS.md), so each availability state renders
/// its message.
class HistorySection extends StatelessWidget {
  /// Creates the section for [availability].
  const new({required this.availability, super.key});

  /// Whether history can be used.
  final Availability availability;

  @override
  Widget build(BuildContext context) {
    final l10n = NotesLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    final (icon, message) = switch (availability) {
      Availability.available => (Icons.history, l10n.historyAvailable),
      Availability.offline => (Icons.cloud_off_outlined, l10n.historyOffline),
      Availability.notYetAvailable => (
        Icons.hourglass_empty,
        l10n.historyNotYetAvailable,
      ),
      Availability.notAllowed => (Icons.block, l10n.historyNotAllowed),
    };
    return Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.historyTitle,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Semantics(
            header: true,
            child: Text(
              l10n.historyTitle,
              style: text.caption
                  .copyWith(color: colors.text2)
                  .copyWith(fontWeight: FontWeight.w600),
            ),
          ),
          const SizedBox(height: StrataSpacing.s2),
          Row(
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
          ),
        ],
      ),
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
        LocalGraphSlot(onOpenMap: widget.onOpenMap),
        const SizedBox(height: StrataSpacing.s3),
        Divider(color: colors.border, height: 1),
        const SizedBox(height: StrataSpacing.s3),
        HistorySection(availability: note.history),
      ],
      ContextTab.graph => [
        LocalGraphSlot(onOpenMap: widget.onOpenMap, height: 280),
      ],
      ContextTab.history => [HistorySection(availability: note.history)],
    };
    return ColoredBox(
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
                        ContextTab.backlinks => l10n.tabBacklinks,
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
  const new({
    required this.label,
    required this.selected,
    required this.onTap,
  });

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
