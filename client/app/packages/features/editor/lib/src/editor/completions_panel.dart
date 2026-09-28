import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';

/// The document layer that anchors the completions popup to the caret: an
/// invisible target at the caret's rect, with the [CompletionsPopup] shown
/// in the app's overlay below it (so it is never clipped by the note's
/// scroll view) while the core has completions for the caret.
class CompletionsCaretLayer extends DocumentLayoutLayerStatefulWidget {
  /// Creates the layer for [controller].
  const new({required this.controller, required this.composer, super.key});

  /// The editing session (its completions).
  final NoteEditorController controller;

  /// The editor's composer (the caret).
  final DocumentComposer composer;

  @override
  DocumentLayoutLayerState<CompletionsCaretLayer, Rect?> createState() =>
      _CompletionsCaretLayerState();
}

class _CompletionsCaretLayerState
    extends DocumentLayoutLayerState<CompletionsCaretLayer, Rect?> {
  final OverlayPortalController _portal = OverlayPortalController();
  final LayerLink _link = LayerLink();

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_sync);
    SchedulerBinding.instance.addPostFrameCallback((_) => _sync());
  }

  @override
  void didUpdateWidget(CompletionsCaretLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_sync);
      widget.controller.addListener(_sync);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_sync);
    super.dispose();
  }

  void _sync() {
    if (!mounted) return;
    final show = widget.controller.completions != null;
    if (show == _portal.isShowing) return;
    if (SchedulerBinding.instance.schedulerPhase ==
        SchedulerPhase.persistentCallbacks) {
      SchedulerBinding.instance.addPostFrameCallback((_) => _sync());
      return;
    }
    show ? _portal.show() : _portal.hide();
  }

  @override
  Rect? computeLayoutDataWithDocumentLayout(
    BuildContext contentLayersContext,
    BuildContext documentContext,
    DocumentLayout documentLayout,
  ) {
    final selection = widget.composer.selection;
    if (selection == null) return null;
    if (documentLayout.getComponentByNodeId(selection.extent.nodeId) == null) {
      return null;
    }
    return documentLayout.getEdgeForPosition(selection.extent);
  }

  @override
  Widget doBuild(BuildContext context, Rect? caret) {
    if (caret == null) return const SizedBox.shrink();
    return Stack(
      clipBehavior: Clip.none,
      children: [
        Positioned(
          left: caret.left,
          top: caret.top,
          width: 1,
          height: caret.height,
          child: CompositedTransformTarget(
            link: _link,
            child: OverlayPortal(
              controller: _portal,
              overlayChildBuilder: (_) => CompositedTransformFollower(
                link: _link,
                showWhenUnlinked: false,
                targetAnchor: Alignment.bottomLeft,
                offset: const Offset(0, StrataSpacing.s1),
                child: Align(
                  alignment: Alignment.topLeft,
                  child: Directionality(
                    textDirection: Directionality.of(context),
                    child: CompletionsPopup(controller: widget.controller),
                  ),
                ),
              ),
              child: const SizedBox.expand(),
            ),
          ),
        ),
      ],
    );
  }
}

/// The completions the core offers at the caret (`editor_completions`):
/// notes for `[[`, people and companies for `@`, tags for `#`, blocks for
/// `[[Note#^`. Choosing an item applies it (the core's `insert_text`, or
/// `insert_mention` for a mention).
class CompletionsPopup extends StatelessWidget {
  /// Creates the popup for [controller]'s completions.
  const new({required this.controller, super.key});

  /// The editing session.
  final NoteEditorController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final completions = controller.completions;
        if (completions == null) return const SizedBox.shrink();
        final heading = switch (completions.kind) {
          CompletionKind.wikiLink => l10n.suggestionsKindWikiLink,
          CompletionKind.mention => l10n.suggestionsKindMention,
          CompletionKind.tag => l10n.suggestionsKindTag,
          CompletionKind.blockRef => l10n.suggestionsKindBlock,
          CompletionKind.none => '',
        };
        return Semantics(
          container: true,
          explicitChildNodes: true,
          label: l10n.suggestionsLabel,
          child: Material(
            color: colors.surface,
            shape: RoundedRectangleBorder(
              borderRadius: StrataRadii.cardRadius,
              side: BorderSide(color: colors.border),
            ),
            clipBehavior: Clip.antiAlias,
            child: DecoratedBox(
              decoration: const BoxDecoration(
                borderRadius: StrataRadii.cardRadius,
                boxShadow: StrataElevation.popover,
              ),
              child: ConstrainedBox(
                constraints: const BoxConstraints(
                  maxWidth: 320,
                  maxHeight: 280,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: Padding(
                            padding: const EdgeInsetsDirectional.only(
                              start: StrataSpacing.s4,
                            ),
                            child: Text(
                              heading,
                              style: text.caption
                                  .withWeight(FontWeight.w600)
                                  .copyWith(color: colors.text2),
                            ),
                          ),
                        ),
                        IconButton(
                          tooltip: l10n.suggestionsDismiss,
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: controller.dismissCompletions,
                        ),
                      ],
                    ),
                    if (completions.items.isEmpty)
                      Padding(
                        padding: const EdgeInsets.fromLTRB(
                          StrataSpacing.s4,
                          0,
                          StrataSpacing.s4,
                          StrataSpacing.s3,
                        ),
                        child: Text(
                          l10n.suggestionsNone,
                          style: text.bodySmall.copyWith(color: colors.text2),
                        ),
                      )
                    else
                      Flexible(
                        child: ListView(
                          shrinkWrap: true,
                          padding: const EdgeInsets.only(
                            bottom: StrataSpacing.s1,
                          ),
                          children: [
                            for (final item in completions.items)
                              CompletionTile(
                                item: item,
                                kind: completions.kind,
                                onTap: () =>
                                    unawaited(controller.applyCompletion(item)),
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
      },
    );
  }
}

/// The glyph of a completion (1:1 on the completion kind and the core's
/// entity kind).
NodeKind completionGlyph(CompletionKind kind, String? entityKind) =>
    switch ((kind, entityKind)) {
      (CompletionKind.mention, 'company') => NodeKind.company,
      (CompletionKind.mention, 'document') => NodeKind.document,
      (CompletionKind.mention, 'place') => NodeKind.place,
      (CompletionKind.mention, _) => NodeKind.person,
      (CompletionKind.tag, _) => NodeKind.concept,
      _ => NodeKind.note,
    };

/// One completion: glyph, label (in its own direction) and detail.
class CompletionTile extends StatelessWidget {
  /// Creates the tile.
  const new({
    required this.item,
    required this.kind,
    required this.onTap,
    super.key,
  });

  /// The item.
  final CompletionItem item;

  /// The completion kind (glyph).
  final CompletionKind kind;

  /// Applies the item.
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
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
                NodeKindGlyph(
                  kind: completionGlyph(kind, item.entityKind),
                  decorative: true,
                ),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        item.label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        textDirection: textDirectionOf(item.labelDir),
                        textAlign: TextAlign.start,
                        style: text.bodySmall.copyWith(
                          color: colors.text,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                      if (item.detail.isNotEmpty)
                        Text(
                          item.detail,
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
