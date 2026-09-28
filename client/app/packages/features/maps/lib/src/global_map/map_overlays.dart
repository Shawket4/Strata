import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_maps/src/graph/graph_camera.dart';
import 'package:strata_maps/src/graph/graph_kinds.dart';
import 'package:strata_maps/src/graph/graph_scene.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The expanded map header: title, counts, lens and search.
class MapHeader extends StatelessWidget {
  /// Creates the header.
  const new({
    required this.counts,
    required this.search,
    required this.lens,
    required this.onLens,
    super.key,
  });

  /// "N nodes · M edges · K clusters".
  final String counts;

  /// The current lens.
  final GraphLens lens;

  /// Switches the lens.
  final ValueChanged<GraphLens> onLens;

  /// The search-to-focus field.
  final Widget search;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s5,
        vertical: StrataSpacing.s2,
      ),
      child: Wrap(
        spacing: StrataSpacing.s4,
        runSpacing: StrataSpacing.s2,
        crossAxisAlignment: WrapCrossAlignment.center,
        alignment: WrapAlignment.spaceBetween,
        children: [
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Semantics(
                header: true,
                container: true,
                child: Text(l10n.mapTitle, style: text.titleSmall),
              ),
              Text(counts, style: text.caption.copyWith(color: colors.text2)),
            ],
          ),
          LensSelector(lens: lens, onChanged: onLens),
          SizedBox(width: 280, child: search),
        ],
      ),
    );
  }
}

/// The lens selector (Notes / People / Companies): the core builds the
/// graph for the lens (`GraphFilter.lens`).
class LensSelector extends StatelessWidget {
  /// Creates the selector.
  const new({required this.lens, required this.onChanged, super.key});

  /// The current lens.
  final GraphLens lens;

  /// Switches the lens.
  final ValueChanged<GraphLens> onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    return Semantics(
      label: l10n.lensLabel,
      container: true,
      child: SegmentedButton<GraphLens>(
        showSelectedIcon: false,
        segments: [
          ButtonSegment(value: GraphLens.notes, label: Text(l10n.lensNotes)),
          ButtonSegment(value: GraphLens.people, label: Text(l10n.lensPeople)),
          ButtonSegment(
            value: GraphLens.companies,
            label: Text(l10n.lensCompanies),
          ),
        ],
        selected: {lens},
        onSelectionChanged: (value) => onChanged(value.first),
      ),
    );
  }
}

/// Search-to-focus: local keyword search (the core's), results in a dropdown;
/// choosing one focuses that node on the map.
class MapSearchField extends HookConsumerWidget {
  /// Creates the field.
  const new({required this.onFocusNode, required this.focusNode, super.key});

  /// Focuses a node (its note ID).
  final ValueChanged<String> onFocusNode;

  /// Keyboard focus of the field (`/` focuses it).
  final FocusNode focusNode;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final controller = useTextEditingController();
    final query = useState('');
    final focused = useListenableSelector(focusNode, () => focusNode.hasFocus);
    final results = query.value.isEmpty
        ? null
        : ref.watch(searchProvider(query.value, SearchMode.keyword));
    final hits = results?.value?.results;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          controller: controller,
          focusNode: focusNode,
          onChanged: (value) => query.value = value,
          decoration: InputDecoration(
            isDense: true,
            hintText: l10n.searchToFocus,
            prefixIcon: const Icon(Icons.search, size: 20),
            suffixIcon: const Padding(
              padding: EdgeInsetsDirectional.only(end: StrataSpacing.s2),
              child: ExcludeSemantics(child: KeyboardHintChip(keys: ['/'])),
            ),
            suffixIconConstraints: const BoxConstraints(minHeight: 20),
            fillColor: colors.surface,
          ),
          textInputAction: TextInputAction.search,
        ),
        if (focused && hits != null)
          Material(
            color: colors.surface,
            shape: RoundedRectangleBorder(
              borderRadius: StrataRadii.cardRadius,
              side: BorderSide(color: colors.border),
            ),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 240),
              child: hits.isEmpty
                  ? Padding(
                      padding: const EdgeInsets.all(StrataSpacing.s3),
                      child: Text(
                        l10n.searchNoResults,
                        style: context.strataText.bodySmall.copyWith(
                          color: colors.text2,
                        ),
                      ),
                    )
                  : ListView(
                      shrinkWrap: true,
                      padding: EdgeInsets.zero,
                      children: [
                        for (final hit in hits)
                          ListTile(
                            dense: true,
                            leading: NodeKindGlyph(
                              kind: nodeKindOf(hit.kind),
                              decorative: true,
                            ),
                            title: Text(
                              hit.title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            onTap: () {
                              onFocusNode(hit.noteId);
                              controller.clear();
                              query.value = '';
                              focusNode.unfocus();
                            },
                          ),
                      ],
                    ),
            ),
          ),
      ],
    );
  }
}

/// "Focused on X" with open / local map / clear actions.
class FocusBar extends StatelessWidget {
  /// Creates the bar.
  const new({
    required this.title,
    required this.onClear,
    super.key,
    this.onOpenNote,
    this.onOpenMindMap,
  });

  /// The focused node's title.
  final String title;

  /// Clears the selection.
  final VoidCallback onClear;

  /// Opens the note.
  final VoidCallback? onOpenNote;

  /// Opens its local mind map.
  final VoidCallback? onOpenMindMap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final open = onOpenNote;
    final local = onOpenMindMap;
    return Semantics(
      container: true,
      liveRegion: true,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.surface,
          border: Border.all(color: colors.border),
          borderRadius: StrataRadii.cardRadius,
        ),
        child: Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s3,
            StrataSpacing.s1,
            StrataSpacing.s1,
            StrataSpacing.s1,
          ),
          child: Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: StrataSpacing.s1,
            children: [
              Text(
                l10n.focusedOn(title: title),
                style: context.strataText.bodySmall.withWeight(FontWeight.w600),
              ),
              if (open != null)
                TextButton(onPressed: open, child: Text(l10n.openNote)),
              if (local != null)
                TextButton(onPressed: local, child: Text(l10n.openLocalMap)),
              TextButton.icon(
                onPressed: onClear,
                icon: const Icon(Icons.close, size: 16),
                label: Text(l10n.clearFocus),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Zoom in / out / fit with the zoom level and label density.
class ZoomControls extends StatelessWidget {
  /// Creates the controls for [view].
  const new({required this.view, super.key});

  /// The camera.
  final GraphViewController view;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      mainAxisSize: MainAxisSize.min,
      children: [
        Semantics(
          label: l10n.zoomGroup,
          container: true,
          explicitChildNodes: true,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border.all(color: colors.border),
              borderRadius: StrataRadii.cardRadius,
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  tooltip: l10n.zoomIn,
                  onPressed: () => view.zoomBy(1.25),
                  icon: const Icon(Icons.add),
                ),
                IconButton(
                  tooltip: l10n.zoomOut,
                  onPressed: () => view.zoomBy(0.8),
                  icon: const Icon(Icons.remove),
                ),
                IconButton(
                  tooltip: l10n.zoomToFit,
                  onPressed: view.fit,
                  icon: const Icon(Icons.fit_screen_outlined),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(width: StrataSpacing.s2),
        ListenableBuilder(
          listenable: view,
          builder: (context, _) {
            final camera = view.camera;
            final percent = (camera.scale * 100).round();
            final label = switch (camera.band) {
              ZoomBand.far => l10n.zoomLevelFar(percent: percent),
              ZoomBand.mid => l10n.zoomLevelMid(percent: percent),
              ZoomBand.near => l10n.zoomLevelNear(percent: percent),
            };
            return StatusPill(label: label, icon: Icons.zoom_in_map);
          },
        ),
      ],
    );
  }
}

/// A node's hover card: kind, title and link count.
class HoverCard extends StatelessWidget {
  /// Creates the card for [node] near [position].
  const new({required this.node, required this.position, super.key});

  /// The hovered node.
  final GraphNode node;

  /// Pointer position in the canvas.
  final Offset position;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final kind = nodeKindOf(node.kind.name);
    return Positioned(
      left: position.dx + 14,
      top: position.dy + 14,
      width: 240,
      child: IgnorePointer(
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: colors.surface,
            border: Border.all(color: colors.border),
            borderRadius: StrataRadii.cardRadius,
            boxShadow: StrataElevation.popover,
          ),
          child: Padding(
            padding: const EdgeInsets.all(StrataSpacing.s3),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  children: [
                    NodeKindGlyph(kind: kind, decorative: true),
                    const SizedBox(width: StrataSpacing.s2),
                    Text(
                      context.l10n.nodeKindLabel(kind),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s1),
                Text(
                  node.title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  textDirection: textDirectionOf(node.titleDir),
                  style: text.bodySmall.withWeight(FontWeight.w600),
                ),
                if (node.summary case final summary?)
                  Text(
                    summary,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: text.caption.copyWith(color: colors.text),
                  ),
                Text(
                  node.updatedLabel.isEmpty
                      ? l10n.hoverLinks(count: node.degree)
                      : l10n.hoverLinksUpdated(
                          links: l10n.hoverLinks(count: node.degree),
                          updated: node.updatedLabel,
                        ),
                  style: text.caption.copyWith(color: colors.text2),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The expanded map's minimap: every node as a dot and the viewport frame;
/// tapping centres the map there.
class Minimap extends StatefulWidget {
  /// Creates the minimap of [scene] following [view].
  const new({required this.scene, required this.view, super.key});

  /// The graph.
  final GraphScene scene;

  /// The main map camera.
  final GraphViewController view;

  @override
  State<Minimap> createState() => _MinimapState();
}

class _MinimapState extends State<Minimap> {
  static const Size _size = Size(160, 124);

  GraphCamera get _camera =>
      GraphCamera.fit(widget.scene.bounds, _size, padding: 8);

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Semantics(
      label: context.mapsL10n.minimap,
      image: true,
      child: GestureDetector(
        onTapUp: (details) =>
            widget.view.centreOn(_camera.toWorld(details.localPosition)),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: colors.surface,
            border: Border.all(color: colors.border),
            borderRadius: StrataRadii.cardRadius,
          ),
          child: CustomPaint(
            size: _size,
            painter: _MinimapPainter(
              scene: widget.scene,
              view: widget.view,
              camera: _camera,
              dot: colors.text2,
              frame: context.strataGraphColors.selectedRing,
            ),
          ),
        ),
      ),
    );
  }
}

class _MinimapPainter extends CustomPainter {
  new({
    required this.scene,
    required this.view,
    required this.camera,
    required this.dot,
    required this.frame,
  }) : super(repaint: view);

  final GraphScene scene;
  final GraphViewController view;
  final GraphCamera camera;
  final Color dot;
  final Color frame;

  @override
  void paint(Canvas canvas, Size size) {
    final n = scene.nodeCount;
    final points = <Offset>[
      for (var i = 0; i < n; i++) camera.toScreen(scene.positionOf(i)),
    ];
    canvas
      ..save()
      ..clipRect(Offset.zero & size)
      ..drawPoints(
        ui.PointMode.points,
        points,
        Paint()
          ..color = dot
          ..strokeWidth = 2
          ..strokeCap = StrokeCap.round,
      );
    final main = view.camera;
    final viewport = view.viewport;
    final topLeft = camera.toScreen(main.toWorld(Offset.zero));
    final bottomRight = camera.toScreen(
      main.toWorld(Offset(viewport.width, viewport.height)),
    );
    canvas
      ..drawRect(
        Rect.fromPoints(topLeft, bottomRight),
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..color = frame,
      )
      ..restore();
  }

  @override
  bool shouldRepaint(_MinimapPainter oldDelegate) =>
      !identical(oldDelegate.scene, scene) ||
      oldDelegate.dot != dot ||
      oldDelegate.frame != frame ||
      oldDelegate.camera != camera;
}
