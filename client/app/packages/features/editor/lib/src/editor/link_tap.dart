import 'package:flutter/widgets.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/source/source_document.dart';
import 'package:strata_ui/strata_ui.dart' show OpenNoteAt;
import 'package:super_editor/super_editor.dart';

/// Opens a wikilink or embed when it is clicked in interaction mode
/// (⌘/Ctrl held on desktop), like Obsidian's source mode. A plain tap places
/// the caret so links stay editable; on phones the toolbar offers "Open
/// link" for the link under the caret. The target (note and block or
/// heading) is the one the core resolved for the hint.
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

  /// Opens the link's target note at its anchor.
  final OpenNoteAt onOpenLink;

  @override
  void dispose() {
    composer.isInInteractionMode.removeListener(notifyListeners);
    super.dispose();
  }

  LineSpan? _linkAt(DocumentPosition? position) {
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
    final target = link?.targetId;
    if (link == null || target == null) {
      return TapHandlingInstruction.continueHandling;
    }
    onOpenLink(target, link.targetAnchor);
    return TapHandlingInstruction.halt;
  }
}
