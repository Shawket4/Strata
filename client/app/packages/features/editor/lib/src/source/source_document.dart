import 'package:flutter/foundation.dart' show immutable;
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

/// A hint span inside one line (UTF-16 offsets relative to the line).
@immutable
final class LineSpan {
  /// Creates a span of [kind] from [start] to [end].
  const new(this.kind, this.start, this.end);

  /// What the span is.
  final HintKind kind;

  /// Start (inclusive).
  final int start;

  /// End (exclusive).
  final int end;

  @override
  bool operator ==(Object other) =>
      other is LineSpan &&
      other.kind == kind &&
      other.start == start &&
      other.end == end;

  @override
  int get hashCode => Object.hash(kind, start, end);

  @override
  String toString() => 'LineSpan($kind, $start, $end)';
}

/// The core's hints of one content, placed on the document's lines.
@immutable
final class LineHints {
  /// Creates line hints.
  const new({this.spans = const {}, this.taskIds = const {}});

  /// Places [hints] (offsets into the note's content) on the lines of
  /// [document], whose body starts at [bodyOffset]. [lines] are the
  /// document's lines in node order (see [linesOf]).
  ///
  /// A task line's ID is the text of the block-ID hint inside the core's
  /// task-line hint, without its `^` (the core does not attach the ID to
  /// the hint yet: docs/CORE_GAPS.md).
  factory place({
    required Document document,
    required List<SourceLine> lines,
    required String newline,
    required int bodyOffset,
    required List<EditorHint> hints,
  }) {
    final spans = <String, List<LineSpan>>{};
    final taskIds = <String, String>{};
    final starts = <int>[];
    final ids = <String>[];
    var offset = bodyOffset;
    var index = 0;
    for (final node in document) {
      starts.add(offset);
      ids.add(node.id);
      final line = lines[index];
      offset += line.text.length;
      if (index < lines.length - 1) {
        offset += line.eol.isEmpty ? newline.length : line.eol.length;
      }
      index++;
    }
    for (final hint in hints) {
      if (hint.kind == HintKind.frontmatter) continue;
      for (var i = 0; i < ids.length; i++) {
        final lineStart = starts[i];
        final lineEnd = lineStart + lines[i].text.length;
        if (hint.end <= lineStart || hint.start > lineEnd) continue;
        if (hint.start == lineEnd && hint.end > lineEnd) continue;
        final start = (hint.start - lineStart).clamp(0, lines[i].text.length);
        final end = (hint.end - lineStart).clamp(0, lines[i].text.length);
        if (end <= start) continue;
        (spans[ids[i]] ??= []).add(LineSpan(hint.kind, start, end));
      }
    }
    for (final entry in spans.entries) {
      final isTask = entry.value.any((s) => s.kind == HintKind.taskLine);
      if (!isTask) continue;
      final line = lines[ids.indexOf(entry.key)].text;
      for (final span in entry.value) {
        if (span.kind == HintKind.blockId && span.end - span.start > 1) {
          taskIds[entry.key] = line.substring(span.start + 1, span.end);
        }
      }
    }
    return LineHints(spans: spans, taskIds: taskIds);
  }

  /// Spans per node ID.
  final Map<String, List<LineSpan>> spans;

  /// Task block IDs per node ID of a task line.
  final Map<String, String> taskIds;

  /// The spans of the line [nodeId].
  List<LineSpan> of(String nodeId) => spans[nodeId] ?? const [];

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
