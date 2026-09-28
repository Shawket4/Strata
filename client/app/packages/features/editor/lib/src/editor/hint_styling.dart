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

/// Hides a markdown marker (`**`, `#`, `[[`…) of a line the caret is not
/// on, in live preview. The characters stay in the document (the source is
/// never rewritten); they are laid out with no width and no colour.
const NamedAttribution hiddenMarkerAttribution = NamedAttribution(
  'strata.marker.hidden',
);

/// The marker ranges of [span] (line offsets), from the core's span bounds
/// and the fixed marker width of its kind: `**`/`__`, `~~`, `==` (2), `*`/`_`
/// (1), `[[`…`]]`, `![[`…`]]`, and a heading's `#`s and space (its level
/// plus one). Nothing is parsed: a span too short for its markers has none.
List<(int, int)> markerRanges(LineSpan span) {
  final (open, close) = switch (span.kind) {
    HintKind.bold || HintKind.strike || HintKind.mark => (2, 2),
    HintKind.italic => (1, 1),
    HintKind.wikiLink => (2, 2),
    HintKind.embed => (3, 2),
    HintKind.heading when span.level > 0 => (span.level + 1, 0),
    _ => (0, 0),
  };
  if (open + close == 0 || span.end - span.start <= open + close) return [];
  return [
    (span.start, span.start + open),
    if (close > 0) (span.end - close, span.end),
  ];
}

/// Adds the core's hints (and task states) of each line to the laid-out
/// text: the line's direction (`RtlLine` / `LtrLine`), the styled spans and,
/// in live preview, the hidden markers of every line but the caret's.
/// Re-runs whenever the controller's hints, the note's tasks, the caret's
/// line or the preview mode change.
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
    final direction =
        controller.lineHints.directionOf(component.nodeId) ?? TextDirection.ltr;
    textModel
      ..textDirection = direction
      ..textAlignment = direction == TextDirection.rtl
          ? TextAlign.right
          : TextAlign.left;
    final hideMarkers =
        controller.livePreview && controller.caretNodeId != component.nodeId;
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
      if (!hideMarkers) continue;
      for (final (from, to) in markerRanges(span)) {
        if (to > length || from >= to) continue;
        text.addAttribution(
          hiddenMarkerAttribution,
          SpanRange(from, to - 1),
          overwriteConflictingSpans: true,
        );
      }
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
  if (attributions.contains(hiddenMarkerAttribution)) {
    // Laid out with (almost) no width: the caret still moves through it.
    return existing.copyWith(
      fontSize: 0.01,
      letterSpacing: 0,
      color: const Color(0x00000000),
      backgroundColor: const Color(0x00000000),
    );
  }
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
