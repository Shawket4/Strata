import 'package:flutter/material.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/source/source_document.dart';
import 'package:strata_state/strata_state.dart' show HintKind, TaskState;
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';

/// A core editor hint applied to a range of a line (view-only: added to the
/// laid-out text, never stored in the document).
@immutable
final class HintAttribution implements Attribution {
  /// Creates the attribution of [kind].
  const new(this.kind);

  /// The hint kind.
  final HintKind kind;

  @override
  String get id => 'strata.hint.${kind.name}';

  @override
  bool canMergeWith(Attribution other) => this == other;

  @override
  bool operator ==(Object other) =>
      other is HintAttribution && other.kind == kind;

  @override
  int get hashCode => kind.hashCode;
}

/// Marks a done or cancelled task line (struck through, secondary colour).
const NamedAttribution taskClosedAttribution = NamedAttribution(
  'strata.task.closed',
);

/// The direction of a line by its first strong character (Unicode bidi rule
/// P2, what `dir="auto"` does): markdown markers, digits and punctuation
/// before the first letter do not count, so `- خصم` and `## الفرضيات` are
/// right-to-left. `super_editor` only looks at the first non-space
/// character. Lines without a strong character are left-to-right.
///
/// This is text layout, not note logic; the core does not stream line
/// directions yet (docs/CORE_GAPS.md).
TextDirection firstStrongDirection(String text) {
  for (final rune in text.runes) {
    if (_isRtl(rune)) return TextDirection.rtl;
    if (_strongLtr.hasMatch(String.fromCharCode(rune))) {
      return TextDirection.ltr;
    }
  }
  return TextDirection.ltr;
}

bool _isRtl(int rune) =>
    (rune >= 0x0590 && rune <= 0x08FF) ||
    (rune >= 0xFB1D && rune <= 0xFDFF) ||
    (rune >= 0xFE70 && rune <= 0xFEFF) ||
    (rune >= 0x10800 && rune <= 0x10FFF) ||
    (rune >= 0x1E800 && rune <= 0x1EFFF);

final RegExp _strongLtr = RegExp(r'\p{L}', unicode: true);

/// Adds the core's hints (and task states) of each line to the laid-out
/// text. Re-runs whenever the controller's hints or the note's tasks change.
final class HintStylePhase extends SingleColumnLayoutStylePhase {
  /// Creates the phase for [controller].
  new(this.controller) {
    _generation = controller.generation;
    controller.addListener(_onController);
  }

  /// The editing session whose hints are shown.
  final NoteEditorController controller;

  int _generation = 0;

  void _onController() {
    if (controller.generation == _generation) return;
    _generation = controller.generation;
    markDirty();
  }

  @override
  void dispose() {
    controller.removeListener(_onController);
    super.dispose();
  }

  @override
  SingleColumnLayoutViewModel style(
    Document document,
    SingleColumnLayoutViewModel viewModel,
  ) {
    final hints = controller.lineHints;
    return SingleColumnLayoutViewModel(
      padding: viewModel.padding,
      componentViewModels: [
        for (final component in viewModel.componentViewModels)
          _style(component, hints.of(component.nodeId)),
      ],
    );
  }

  SingleColumnLayoutComponentViewModel _style(
    SingleColumnLayoutComponentViewModel component,
    List<LineSpan> spans,
  ) {
    if (component is! TextComponentViewModel) return component;
    final copy = component.copy();
    final textModel = copy as TextComponentViewModel;
    final text = textModel.text.copy();
    final length = text.length;
    final direction = firstStrongDirection(text.toPlainText());
    textModel
      ..textDirection = direction
      ..textAlignment = direction == TextDirection.rtl
          ? TextAlign.right
          : TextAlign.left;
    for (final span in spans) {
      final end = span.end.clamp(0, length);
      final start = span.start.clamp(0, end);
      if (end <= start) continue;
      text.addAttribution(
        HintAttribution(span.kind),
        // Attribution ranges are end-inclusive.
        SpanRange(start, end - 1),
        overwriteConflictingSpans: true,
      );
    }
    final task = controller.taskOf(component.nodeId);
    if (task != null && task.state != TaskState.open && length > 0) {
      text.addAttribution(taskClosedAttribution, SpanRange(0, length - 1));
    }
    textModel.text = text;
    return copy;
  }
}

/// The editor's stylesheet in the Strata tokens: body text in Cairo 16/1.5
/// with the core's hints styled (headings, links, tags, block IDs, code).
Stylesheet strataEditorStylesheet(BuildContext context, {EdgeInsets? padding}) {
  final colors = context.strataColors;
  final text = context.strataText;
  final base = text.body.copyWith(color: colors.text);
  return Stylesheet(
    documentPadding: padding ?? EdgeInsets.zero,
    rules: [
      StyleRule(
        BlockSelector.all,
        (document, node) => {
          Styles.maxWidth: double.infinity,
          Styles.padding: const CascadingPadding.only(bottom: 2),
          Styles.textStyle: base,
        },
      ),
    ],
    inlineTextStyler: (attributions, existing) =>
        hintTextStyle(context, attributions, existing),
    selectedTextColorStrategy: ({
      required originalTextColor,
      required selectionHighlightColor,
    }) => originalTextColor,
  );
}

/// The style of a run with [attributions] (1:1 from hint kinds to tokens).
TextStyle hintTextStyle(
  BuildContext context,
  Set<Attribution> attributions,
  TextStyle existing,
) {
  final colors = context.strataColors;
  final text = context.strataText;
  var style = existing;
  for (final attribution in attributions) {
    if (attribution == taskClosedAttribution) {
      style = style.copyWith(
        color: colors.text2,
        decoration: TextDecoration.lineThrough,
        decorationColor: colors.text2,
      );
    }
    if (attribution is! HintAttribution) continue;
    style = switch (attribution.kind) {
      HintKind.heading => style.copyWith(
        fontSize: StrataTypeScale.title - 2,
        fontWeight: FontWeight.w600,
        height: 1.4,
      ),
      HintKind.wikiLink || HintKind.embed => style.copyWith(
        color: colors.accentText,
        decoration: TextDecoration.underline,
        decorationColor: colors.accentText,
      ),
      HintKind.tag => style.copyWith(
        color: colors.infoText,
        backgroundColor: colors.infoTint,
      ),
      HintKind.blockId => text.monoSmall.copyWith(
        color: colors.text2,
        fontSize: existing.fontSize == null ? null : 13,
      ),
      HintKind.code => text.mono.copyWith(
        color: colors.text,
        backgroundColor: colors.surface2,
      ),
      HintKind.bold => style.copyWith(fontWeight: FontWeight.w700),
      HintKind.italic => style.copyWith(fontStyle: FontStyle.italic),
      HintKind.strike => style.copyWith(decoration: TextDecoration.lineThrough),
      HintKind.mark => style.copyWith(backgroundColor: colors.warningTint),
      HintKind.frontmatter ||
      HintKind.taskLine ||
      HintKind.rtlLine ||
      HintKind.ltrLine => style,
    };
  }
  return style;
}
