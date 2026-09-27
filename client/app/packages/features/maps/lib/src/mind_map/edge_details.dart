import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A relation's details (compact bottom sheet, wider hover card): type, AI
/// confidence, ends, the AI's reason, and Retype / Reject for typed
/// relations (D13: both forward intents to the core).
class EdgeDetails extends ConsumerWidget {
  /// Creates the details of [edge] between [from] and [to].
  const new({
    required this.edge,
    required this.from,
    required this.to,
    required this.onClose,
    super.key,
  });

  /// The edge.
  final GraphEdge edge;

  /// Source title.
  final String from;

  /// Target title.
  final String to;

  /// Closes the sheet / card.
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final shared = context.l10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final cls = edgeClassOf(edge.kind);
    final type = relationTypeOf(cls);
    final wire = relationWireTypes[cls];
    final confidence = edge.by == 'ai' ? edge.confidence : null;
    return Semantics(
      container: true,
      label: l10n.selectedRelation,
      explicitChildNodes: true,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          StrataSpacing.s4,
          0,
          StrataSpacing.s4,
          StrataSpacing.s4,
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Flexible(
                  child: RelationChip(
                    type: type,
                    label: shared.relationTypeLabel(type),
                    aiConfidence: confidence,
                  ),
                ),
                const Spacer(),
                IconButton(
                  tooltip: shared.actionClose,
                  onPressed: onClose,
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            const SizedBox(height: StrataSpacing.s2),
            Semantics(
              header: true,
              container: true,
              child: Text(
                l10n.edgeFromTo(from: from, to: to),
                style: text.bodyStrong,
              ),
            ),
            if (confidence != null) ...[
              const SizedBox(height: StrataSpacing.s3),
              DecoratedBox(
                decoration: BoxDecoration(
                  color: colors.background,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: Padding(
                  padding: const EdgeInsets.all(StrataSpacing.s3),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l10n.whyAi,
                        style: text.caption
                            .withWeight(FontWeight.w700)
                            .copyWith(color: colors.text2),
                      ),
                      const SizedBox(height: StrataSpacing.s1),
                      Text(
                        l10n.reasonUnavailable,
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                    ],
                  ),
                ),
              ),
            ],
            if (wire != null) ...[
              const SizedBox(height: StrataSpacing.s3),
              Row(
                children: [
                  Expanded(
                    child: MenuAnchor(
                      menuChildren: [
                        for (final entry in relationWireTypes.entries)
                          if (entry.value != wire)
                            MenuItemButton(
                              leadingIcon: RelationLineSample(
                                type: relationTypeOf(entry.key),
                              ),
                              onPressed: () {
                                unawaited(
                                  ref
                                      .read(coreApiProvider)
                                      .retypeRelation(
                                        srcId: edge.src,
                                        dstId: edge.dst,
                                        relType: wire,
                                        newType: entry.value,
                                      ),
                                );
                                onClose();
                              },
                              child: Text(
                                shared.relationTypeLabel(
                                  relationTypeOf(entry.key),
                                ),
                              ),
                            ),
                      ],
                      builder: (context, controller, _) => OutlinedButton.icon(
                        onPressed: () => controller.isOpen
                            ? controller.close()
                            : controller.open(),
                        icon: const Icon(Icons.swap_horiz, size: 20),
                        label: Text(l10n.retype),
                        style: OutlinedButton.styleFrom(
                          minimumSize: const Size.fromHeight(48),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(
                    child: OutlinedButton.icon(
                      onPressed: () {
                        unawaited(
                          ref
                              .read(coreApiProvider)
                              .removeRelation(
                                srcId: edge.src,
                                dstId: edge.dst,
                                relType: wire,
                              ),
                        );
                        onClose();
                      },
                      icon: const Icon(Icons.block, size: 20),
                      label: Text(l10n.reject),
                      style: OutlinedButton.styleFrom(
                        minimumSize: const Size.fromHeight(48),
                        foregroundColor: colors.dangerText,
                      ),
                    ),
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
