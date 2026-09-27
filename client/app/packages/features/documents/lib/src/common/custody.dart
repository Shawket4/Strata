import 'package:flutter/material.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/labels.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A document status as a tinted pill.
class DocumentStatusPill extends StatelessWidget {
  /// Creates the pill for [status].
  const new(this.status, {super.key});

  /// The core's status string.
  final String status;

  @override
  Widget build(BuildContext context) {
    final shown = documentStatus(context.docsL10n, status);
    return StatusPill(label: shown.label, tone: shown.tone, icon: shown.icon);
  }
}

/// A place breadcrumb, outermost first, each place a link (`›` mirrors in
/// right-to-left text).
class PlaceBreadcrumb extends StatelessWidget {
  /// Creates the breadcrumb of [places].
  const new(this.places, {super.key, this.style});

  /// Enclosing places, outermost first.
  final List<EntityRef> places;

  /// Text style.
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final base = style ?? context.strataText.body;
    return Semantics(
      label: context.docsL10n.locationLabel,
      container: true,
      explicitChildNodes: true,
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: StrataSpacing.s1,
        children: [
          for (var i = 0; i < places.length; i++) ...[
            if (i > 0)
              ExcludeSemantics(
                child: Text('›', style: base.copyWith(color: colors.text2)),
              ),
            EntityLink(places[i], style: base),
          ],
        ],
      ),
    );
  }
}

/// Who holds a document: "With X", or "Nobody has it now · last with Y".
class HolderLine extends StatelessWidget {
  /// Creates the line.
  const new({required this.holder, required this.lastHolder, super.key});

  /// Current holder.
  final EntityRef? holder;

  /// Last holder.
  final EntityRef? lastHolder;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final style = context.strataText.bodySmall.copyWith(color: colors.text2);
    final now = holder;
    final last = lastHolder;
    return Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: StrataSpacing.s1,
      children: [
        if (now != null) ...[
          Text(l10n.withLabel, style: style),
          EntityLink(now),
        ] else ...[
          Text(
            l10n.nobodyHasIt,
            style: style.withWeight(FontWeight.w600).copyWith(
              color: colors.text,
            ),
          ),
          if (last != null) ...[
            Text(l10n.lastWith, style: style),
            EntityLink(last),
          ],
        ],
      ],
    );
  }
}

/// The "Where it is" card: status, place breadcrumb, holder, and the
/// "Record a move" action.
class WhereItIsCard extends StatelessWidget {
  /// Creates the card for [document].
  const new({required this.document, required this.onRecordMove, super.key});

  /// The document.
  final DocumentView document;

  /// Opens the record-a-move form.
  final VoidCallback onRecordMove;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final status = document.status;
    final compact = SizeClass.of(context) == SizeClass.compact;
    final details = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: StrataSpacing.s2,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Semantics(
              header: true,
              child: Text(
                l10n.whereItIs,
                style: text.caption
                    .withWeight(FontWeight.w700)
                    .copyWith(color: colors.text2),
              ),
            ),
            if (status != null) DocumentStatusPill(status),
          ],
        ),
        const SizedBox(height: StrataSpacing.s1),
        if (document.location.isEmpty)
          Text(l10n.locationUnknown, style: text.titleSmall)
        else
          PlaceBreadcrumb(document.location, style: text.titleSmall),
        const SizedBox(height: StrataSpacing.s1),
        HolderLine(holder: document.holder, lastHolder: document.lastHolder),
      ],
    );
    final button = FilledButton.icon(
      onPressed: onRecordMove,
      icon: const Icon(Icons.swap_horiz, size: 20),
      label: Text(l10n.recordMove),
      style: FilledButton.styleFrom(minimumSize: const Size(48, 48)),
    );
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        child: compact
            ? Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const KindAvatar(kind: NodeKind.place),
                      const SizedBox(width: StrataSpacing.s3),
                      Expanded(child: details),
                    ],
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  button,
                ],
              )
            : Wrap(
                spacing: StrataSpacing.s4,
                runSpacing: StrataSpacing.s3,
                crossAxisAlignment: WrapCrossAlignment.center,
                alignment: WrapAlignment.spaceBetween,
                children: [
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const KindAvatar(kind: NodeKind.place, size: 44),
                      const SizedBox(width: StrataSpacing.s3),
                      ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 480),
                        child: details,
                      ),
                    ],
                  ),
                  button,
                ],
              ),
      ),
    );
  }
}

/// One custody event: date · type, the document / place / people it moved
/// between, and its citations.
class CustodyTile extends StatelessWidget {
  /// Creates the tile for [item].
  const new(this.item, {super.key, this.showDocument = false});

  /// The event.
  final CustodyItem item;

  /// Whether the document is named (place pages).
  final bool showDocument;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final kind = custodyKind(l10n, item.kind);
    final palette = kind.tone.colorsIn(colors);
    final document = item.document;
    final place = item.place;
    final person = item.person;
    final counterparty = item.counterparty;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(
              color: palette.background,
              shape: BoxShape.circle,
            ),
            child: Icon(kind.icon, size: 16, color: palette.foreground),
          ),
          const SizedBox(width: StrataSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.custodyHeading(date: item.date, kind: kind.label),
                  style: text.caption.copyWith(color: colors.text2),
                ),
                Wrap(
                  spacing: StrataSpacing.s2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    if (showDocument && document != null)
                      EntityLink(document, style: text.bodySmall),
                    if (place != null) ...[
                      const Icon(Icons.place_outlined, size: 16),
                      EntityLink(place),
                    ],
                    if (person != null) ...[
                      const Icon(Icons.person_outline, size: 16),
                      EntityLink(person),
                    ],
                    if (counterparty != null) ...[
                      const Icon(Icons.business_outlined, size: 16),
                      EntityLink(counterparty),
                    ],
                  ],
                ),
                if (item.citations.isNotEmpty) ...[
                  const SizedBox(height: StrataSpacing.s1),
                  CitationRow(item.citations),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// A list of custody events in a card, newest first (the core's order).
class CustodyList extends StatelessWidget {
  /// Creates the list.
  const new(this.items, {super.key, this.showDocument = false, this.empty});

  /// Events.
  final List<CustodyItem> items;

  /// Whether each event names its document.
  final bool showDocument;

  /// Text when there are no events.
  final String? empty;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    if (items.isEmpty) {
      return Text(
        empty ?? context.docsL10n.noCustody,
        style: context.strataText.bodySmall.copyWith(color: colors.text2),
      );
    }
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s4,
          vertical: StrataSpacing.s2,
        ),
        child: Column(
          children: [
            for (var i = 0; i < items.length; i++) ...[
              if (i > 0) Divider(height: 1, color: colors.border),
              CustodyTile(items[i], showDocument: showDocument),
            ],
          ],
        ),
      ),
    );
  }
}

/// A document row (place pages, directory lists): glyph, title, where and
/// who, status.
class DocumentBriefTile extends StatelessWidget {
  /// Creates the row for [document].
  const new(this.document, {super.key, this.dense = false});

  /// The document.
  final DocumentBrief document;

  /// Denser padding (expanded lists).
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final l10n = context.docsL10n;
    final open = EntityLinks.of(context).onOpenEntity;
    final location = document.location;
    final holder = document.holder;
    final status = document.status;
    return Semantics(
      button: open != null,
      child: InkWell(
        onTap: open == null ? null : () => open(document.id),
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: StrataSpacing.s4,
            vertical: dense ? StrataSpacing.s2 : StrataSpacing.s3,
          ),
          child: Row(
            children: [
              KindAvatar(kind: NodeKind.document, size: dense ? 32 : 40),
              const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      document.title,
                      style: text.body.withWeight(FontWeight.w600),
                    ),
                    if (location != null)
                      Text(
                        location.title,
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                    if (holder != null)
                      Text(
                        l10n.withHolder(name: holder.title),
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                  ],
                ),
              ),
              if (status != null) ...[
                const SizedBox(width: StrataSpacing.s2),
                DocumentStatusPill(status),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
