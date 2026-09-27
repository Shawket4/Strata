import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:strata_editor/src/editor/hint_styling.dart';
import 'package:strata_editor/src/editor/link_tap.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/editor/task_line.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';

/// The markdown editor of a note: `super_editor` over the note's source
/// lines (see `MarkdownSource`), styled from the core's hints, with task-line
/// checkboxes and wikilinks that open on ⌘/Ctrl-click.
///
/// Shows [controller]'s current document; the owner feeds the controller the
/// note views the core streams (`NoteEditorController.show`). The editor is a
/// sliver: place it in a [CustomScrollView] (it scrolls with the note's
/// title and properties above it).
class StrataNoteEditor extends StatefulWidget {
  /// Creates the editor.
  const new({
    required this.controller,
    super.key,
    this.onOpenLink,
    this.onSave,
    this.padding = EdgeInsets.zero,
    this.focusNode,
    this.autofocus = false,
  });

  /// The editing session.
  final NoteEditorController controller;

  /// Receives a wikilink's source text (`[[Note|alias]]`) when it is opened.
  final ValueChanged<String>? onOpenLink;

  /// ⌘/Ctrl-S.
  final VoidCallback? onSave;

  /// Padding around the document.
  final EdgeInsets padding;

  /// The editor's focus node.
  final FocusNode? focusNode;

  /// Whether the editor takes focus when shown.
  final bool autofocus;

  @override
  State<StrataNoteEditor> createState() => _StrataNoteEditorState();
}

class _StrataNoteEditorState extends State<StrataNoteEditor> {
  late HintStylePhase _phase = HintStylePhase(widget.controller);
  Stylesheet? _stylesheet;
  StrataColors? _stylesheetColors;
  EdgeInsets? _stylesheetPadding;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onController);
  }

  @override
  void didUpdateWidget(StrataNoteEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_onController);
      widget.controller.addListener(_onController);
      _phase.dispose();
      _phase = HintStylePhase(widget.controller);
    }
  }

  Editor? _shownEditor;

  void _onController() {
    // Rebuild only when the document is replaced (a new editor); hint and
    // task changes restyle through the style phase.
    if (widget.controller.editor != _shownEditor) setState(() {});
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onController);
    _phase.dispose();
    super.dispose();
  }

  Stylesheet _stylesheetFor(BuildContext context) {
    final colors = context.strataColors;
    final cached = _stylesheet;
    if (cached != null &&
        _stylesheetColors == colors &&
        _stylesheetPadding == widget.padding) {
      return cached;
    }
    _stylesheetColors = colors;
    _stylesheetPadding = widget.padding;
    return _stylesheet = strataEditorStylesheet(
      context,
      padding: widget.padding,
    );
  }

  ExecutionInstruction _shortcuts({
    required SuperEditorContext editContext,
    required KeyEvent keyEvent,
  }) {
    if (keyEvent is! KeyDownEvent) {
      return ExecutionInstruction.continueExecution;
    }
    final keyboard = HardwareKeyboard.instance;
    final primary = keyboard.isMetaPressed || keyboard.isControlPressed;
    final key = keyEvent.logicalKey;
    if (primary && key == LogicalKeyboardKey.keyS) {
      widget.onSave?.call();
      return ExecutionInstruction.haltExecution;
    }
    if (primary && key == LogicalKeyboardKey.keyB) {
      widget.controller.wrapSelection('**');
      return ExecutionInstruction.haltExecution;
    }
    if (primary && key == LogicalKeyboardKey.keyI) {
      widget.controller.wrapSelection('_');
      return ExecutionInstruction.haltExecution;
    }
    if (key == LogicalKeyboardKey.escape && widget.controller.trigger != null) {
      widget.controller.dismissTrigger();
      return ExecutionInstruction.haltExecution;
    }
    return ExecutionInstruction.continueExecution;
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final editor = controller.editor;
    _shownEditor = editor;
    if (editor == null) return const SliverToBoxAdapter();
    final colors = context.strataColors;
    final l10n = EditorLocalizations.of(context);
    return SliverSemantics(
      label: l10n.editorBodyLabel,
      container: true,
      textField: true,
      multiline: true,
      sliver: SuperEditor(
        editor: editor,
        focusNode: widget.focusNode,
        autofocus: widget.autofocus,
        shrinkWrap: true,
        stylesheet: _stylesheetFor(context),
        customStylePhases: [_phase],
        selectionStyle: SelectionStyles(selectionColor: colors.accentTint),
        componentBuilders: [
          TaskLineComponentBuilder(controller),
          ...defaultComponentBuilders,
        ],
        keyboardActions: [
          _shortcuts,
          ...defaultImeKeyboardActions.where(
            (action) => !_richTextOnlyActions.contains(action),
          ),
        ],
        contentTapDelegateFactories: [
          (editContext) => WikilinkTapDelegate(
            controller: controller,
            composer: editContext.composer,
            onOpenLink: (link) => widget.onOpenLink?.call(link),
          ),
        ],
        documentOverlayBuilders: [
          DefaultCaretOverlayBuilder(
            caretStyle: CaretStyle(color: colors.accent),
          ),
          ...defaultSuperEditorDocumentOverlayBuilders.where(
            (builder) => builder is! DefaultCaretOverlayBuilder,
          ),
        ],
      ),
    );
  }
}

/// Keyboard actions that change rich-text state the markdown source does not
/// hold (bold/italic attributions, paragraph indents); ⌘B / ⌘I type `**` /
/// `_` instead.
final Set<SuperEditorKeyboardAction> _richTextOnlyActions = {
  cmdBToToggleBold,
  cmdIToToggleItalics,
  tabToIndentTask,
  shiftTabToUnIndentTask,
  tabToIndentParagraph,
  shiftTabToUnIndentParagraph,
  backspaceToUnIndentTask,
  backspaceToUnIndentParagraph,
  backspaceToConvertTaskToParagraph,
  backspaceToClearParagraphBlockType,
  enterToUnIndentParagraph,
};
