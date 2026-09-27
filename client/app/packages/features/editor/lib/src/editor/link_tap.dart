import 'package:flutter/widgets.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:super_editor/super_editor.dart';

/// Opens a wikilink or embed when it is clicked in interaction mode
/// (⌘/Ctrl held on desktop), like Obsidian's source mode. A plain tap places
/// the caret so links stay editable; on phones the toolbar offers "Open
/// link" for the link under the caret.
final class WikilinkTapDelegate extends ContentTapDelegate {
  /// Creates the delegate.
  new({
    required this.controller,
    required this.composer,
    required this.onOpenLink,
  }) {
    composer.isInInteractionMode.addListener(notifyListeners);
  }

  /// The editing session (hints of each line).
  final NoteEditorController controller;

  /// The composer (interaction mode).
  final DocumentComposer composer;

  /// Receives the link's source text (`[[Note|alias]]`).
  final ValueChanged<String> onOpenLink;

  @override
  void dispose() {
    composer.isInInteractionMode.removeListener(notifyListeners);
    super.dispose();
  }

  String? _linkAt(DocumentPosition? position) {
    if (position == null || !composer.isInInteractionMode.value) return null;
    final nodePosition = position.nodePosition;
    if (nodePosition is! TextNodePosition) return null;
    return controller.linkAt(position.nodeId, nodePosition.offset);
  }

  @override
  MouseCursor? mouseCursorForContentHover(DocumentPosition hoverPosition) =>
      _linkAt(hoverPosition) == null ? null : SystemMouseCursors.click;

  @override
  TapHandlingInstruction onTap(DocumentTapDetails details) {
    final position = details.documentLayout.getDocumentPositionNearestToOffset(
      details.layoutOffset,
    );
    final link = _linkAt(position);
    if (link == null) return TapHandlingInstruction.continueHandling;
    onOpenLink(link);
    return TapHandlingInstruction.halt;
  }
}
