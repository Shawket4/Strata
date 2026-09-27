import 'package:flutter/material.dart';
import 'package:strata_ui/strata_ui.dart';

/// Every shared design-system widget with the spec's sample content, used by
/// the widget matrix tests and the design-system goldens.
class WidgetGallery extends StatelessWidget {
  const new({this.interactive = true, super.key});

  /// Whether chips and pills get tap handlers (tap targets apply).
  final bool interactive;

  static const aiRelation = 'Call 2026-09-12 — Acme';

  @override
  Widget build(BuildContext context) {
    void noop() {}
    final tap = interactive ? noop : null;
    const gap = SizedBox(height: StrataSpacing.s3);
    const wrap = StrataSpacing.s2;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        const Align(
          alignment: AlignmentDirectional.centerStart,
          child: StrataWordmark(),
        ),
        gap,
        const Row(
          children: [
            StrataSymbol(size: 16),
            SizedBox(width: StrataSpacing.s2),
            StrataSymbol(),
            SizedBox(width: StrataSpacing.s2),
            StrataSymbol(size: 48),
          ],
        ),
        gap,
        const SizedBox(height: 56, child: StrataBands()),
        const StrataSectionHeader(title: 'Relations', count: 9),
        Wrap(
          spacing: wrap,
          runSpacing: wrap,
          children: [
            RelationChip(
              type: RelationType.followsUp,
              label: aiRelation,
              aiConfidence: 0.82,
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.partOf,
              label: 'Subscription tiers',
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.contradicts,
              label: 'Discount policy',
              aiConfidence: 0.74,
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.supports,
              label: 'Pricing experiments',
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.related,
              label: 'Churn notes',
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.duplicates,
              label: 'Pricing experiment',
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.similarity,
              label: 'Q4 hiring plan',
              aiConfidence: 0.61,
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.bodyLink,
              label: 'Onboarding checklist v2',
              onPressed: tap,
            ),
            RelationChip(
              type: RelationType.mention,
              label: 'أحمد سمير',
              textDirection: TextDirection.rtl,
              onPressed: tap,
            ),
          ],
        ),
        const StrataSectionHeader(title: 'Node kinds'),
        Wrap(
          spacing: StrataSpacing.s4,
          runSpacing: wrap,
          children: [
            for (final kind in NodeKind.values) NodeKindGlyph(kind: kind),
            const NodeKindGlyph(kind: NodeKind.note, size: 24, selected: true),
          ],
        ),
        const StrataSectionHeader(title: 'Sync'),
        Wrap(
          spacing: wrap,
          runSpacing: wrap,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            SyncPill(
              status: const SyncSynced(lastSync: '14:32'),
              onPressed: tap,
            ),
            SyncPill(status: const SyncOffline(queued: 3), onPressed: tap),
            SyncPill(
              status: const SyncInProgress(done: 12, total: 40),
              onPressed: tap,
            ),
            SyncPill(status: const SyncConflict(count: 1), onPressed: tap),
            SyncPill(
              status: const SyncOffline(queued: 3),
              dense: true,
              onPressed: tap,
            ),
          ],
        ),
        const StrataSectionHeader(title: 'Citations'),
        Wrap(
          spacing: wrap,
          runSpacing: wrap,
          children: [
            CitationChip(
              label: aiRelation,
              blockRef: '^a1b2',
              index: 1,
              onPressed: tap,
            ),
            CitationChip(
              label: 'تجارب التسعير — ملخص',
              textDirection: TextDirection.rtl,
              onPressed: tap,
            ),
          ],
        ),
        const StrataSectionHeader(title: 'Status'),
        const Wrap(
          spacing: wrap,
          runSpacing: wrap,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            StatusPill(label: 'Pending approval'),
            StatusPill(label: 'Provider online', tone: StatusTone.success),
            StatusPill(label: 'AI paused', tone: StatusTone.warning),
            StatusPill(label: 'Disabled', tone: StatusTone.danger),
            StatusPill(label: 'Syncing', tone: StatusTone.info),
            KeyboardHintChip(keys: ['⌘', 'K']),
            KeyboardHintChip(keys: ['Ctrl', 'N']),
          ],
        ),
        gap,
        SizedBox(
          height: MediaQuery.textScalerOf(context).scale(280),
          child: StrataEmptyState(
            title: 'Inbox zero',
            message: 'Captures you make on any device land here.',
            action: interactive
                ? StrataAction(
                    label: 'New capture',
                    icon: Icons.add,
                    onPressed: noop,
                  )
                : null,
          ),
        ),
      ],
    );
  }
}
