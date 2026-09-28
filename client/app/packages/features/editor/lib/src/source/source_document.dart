import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter/painting.dart' show TextDirection;
import 'package:strata_editor/src/source/markdown_source.dart';
import 'package:strata_state/strata_state.dart' show EditorHint, HintKind;
import 'package:super_editor/super_editor.dart';

/// Node metadata key holding the line terminator that precedes a body line
/// in the source (`''` for the first line).
///
/// Keeping the terminator on the line *after* it makes edits local: when a
/// line is split, the new line gets a new terminator while the terminator
/// before the next line stays with that line; when two lines are merged,
/// exactly the terminator between them disappears. Mixed `\n` / `\r\n`
/// files therefore keep every terminator the user did not touch.
const String lineEolKey = 'strataEolBefore';

/// Builds the `super_editor` document of [source]: one paragraph per body
/// line, holding the line's exact text (no markdown parsing: styling comes
/// from the core's hints, see `MarkdownSource`).
MutableDocument documentOf(MarkdownSource source) => MutableDocument(
  nodes: [
    for (var i = 0; i < source.lines.length; i++)
      ParagraphNode(
        id: Editor.createNodeId(),
        text: AttributedText(source.lines[i].text),
        metadata: {lineEolKey: i == 0 ? '' : source.lines[i - 1].eol},
      ),
  ],
);

/// The body lines of [document], in order: each line's text and the
/// terminator that follows it (the next line's preceding terminator; `''`
/// when the next line was created in the editor, which `joinLines` replaces
/// with the note's newline). A non-text node, which the editor never
/// creates from a note, is an empty line.
List<SourceLine> linesOf(Document document) {
  final nodes = document.toList();
  return [
    for (var i = 0; i < nodes.length; i++)
      SourceLine(
        switch (nodes[i]) {
          final TextNode node => node.text.toPlainText(),
          _ => '',
        },
        i == nodes.length - 1
            ? ''
            : (nodes[i + 1].getMetadataValue(lineEolKey) as String?) ?? '',
      ),
  ];
}

/// Where each line of [document] starts in the note's content (UTF-16
/// offsets), in node order: the body starts at [bodyOffset] and every line
/// but the last is followed by its terminator ([newline] when it has none).
/// [lines] are the document's lines (see [linesOf]).
List<int> lineStartsOf(
  Document document,
  List<SourceLine> lines, {
  required String newline,
  required int bodyOffset,
}) {
  final starts = <int>[];
  var offset = bodyOffset;
  for (var i = 0; i < lines.length; i++) {
    starts.add(offset);
    offset += lines[i].text.length;
    if (i < lines.length - 1) {
      offset += lines[i].eol.isEmpty ? newline.length : lines[i].eol.length;
    }
  }
  return starts;
}

/// A hint span inside one line (UTF-16 offsets relative to the line), with
/// what the core attached to it (link target, heading level).
@immutable
final class LineSpan {
  /// Creates a span of [kind] from [start] to [end].
  const new(
    this.kind,
    this.start,
    this.end, {
    this.targetId,
    this.targetAnchor,
    this.level = 0,
  });

  /// What the span is.
  final HintKind kind;

  /// Start (inclusive).
  final int start;

  /// End (exclusive).
  final int end;

  /// Wikilinks and embeds: the note the core resolved the link to.
  final String? targetId;

  /// Wikilinks and embeds: the heading or block (without `^`).
  final String? targetAnchor;

  /// Headings: level 1–6.
  final int level;

  @override
  bool operator ==(Object other) =>
      other is LineSpan &&
      other.kind == kind &&
      other.start == start &&
      other.end == end &&
      other.targetId == targetId &&
      other.targetAnchor == targetAnchor &&
      other.level == level;

  @override
  int get hashCode =>
      Object.hash(kind, start, end, targetId, targetAnchor, level);

  @override
  String toString() => 'LineSpan($kind, $start, $end)';
}

/// The core's hints of one content, placed on the document's lines.
@immutable
final class LineHints {
  /// Creates line hints.
  const new({
    this.spans = const {},
    this.taskIds = const {},
    this.directions = const {},
  });

  /// Places [hints] (offsets into the note's content) on the lines of
  /// [document], whose body starts at [bodyOffset]. [lines] are the
  /// document's lines in node order (see [linesOf]).
  ///
  /// A task line's ID is the core's `EditorHint.task_id`; a line's direction
  /// is the core's `RtlLine` / `LtrLine` span over it (first-strong rule,
  /// computed in the core).
  factory place({
    required Document document,
    required List<SourceLine> lines,
    required String newline,
    required int bodyOffset,
    required List<EditorHint> hints,
  }) {
    final spans = <String, List<LineSpan>>{};
    final taskIds = <String, String>{};
    final directions = <String, TextDirection>{};
    final starts = lineStartsOf(
      document,
      lines,
      newline: newline,
      bodyOffset: bodyOffset,
    );
    final ids = [for (final node in document) node.id];
    for (final hint in hints) {
      if (hint.kind == HintKind.frontmatter) continue;
      for (var i = 0; i < ids.length; i++) {
        final lineStart = starts[i];
        final lineEnd = lineStart + lines[i].text.length;
        if (hint.end <= lineStart || hint.start > lineEnd) continue;
        if (hint.start == lineEnd && hint.end > lineEnd) continue;
        final start = (hint.start - lineStart).clamp(0, lines[i].text.length);
        final end = (hint.end - lineStart).clamp(0, lines[i].text.length);
        if (hint.kind == HintKind.rtlLine) {
          directions[ids[i]] = TextDirection.rtl;
          continue;
        }
        if (hint.kind == HintKind.ltrLine) {
          directions[ids[i]] = TextDirection.ltr;
          continue;
        }
        if (hint.kind == HintKind.taskLine) {
          if (hint.taskId case final id?) taskIds[ids[i]] = id;
        }
        if (end <= start && hint.kind != HintKind.taskLine) continue;
        (spans[ids[i]] ??= []).add(
          LineSpan(
            hint.kind,
            start,
            end,
            targetId: hint.targetId,
            targetAnchor: hint.targetAnchor,
            level: hint.level,
          ),
        );
      }
    }
    return LineHints(spans: spans, taskIds: taskIds, directions: directions);
  }

  /// Spans per node ID.
  final Map<String, List<LineSpan>> spans;

  /// Task block IDs per node ID of a task line.
  final Map<String, String> taskIds;

  /// The core's direction per node ID.
  final Map<String, TextDirection> directions;

  /// The spans of the line [nodeId].
  List<LineSpan> of(String nodeId) => spans[nodeId] ?? const [];

  /// The direction of the line [nodeId] (`null`: the core sent none, e.g. a
  /// line without a strong character or not hinted yet).
  TextDirection? directionOf(String nodeId) => directions[nodeId];

  /// Whether [nodeId] is a task line.
  bool isTaskLine(String nodeId) =>
      of(nodeId).any((s) => s.kind == HintKind.taskLine);

  /// The hint of [kind] covering [offset] of line [nodeId], if any.
  LineSpan? at(String nodeId, int offset, HintKind kind) {
    for (final span in of(nodeId)) {
      if (span.kind == kind && offset >= span.start && offset <= span.end) {
        return span;
      }
    }
    return null;
  }
}
