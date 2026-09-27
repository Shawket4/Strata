/// Byte-exact markdown source model of the editor (PLAN D2 = b).
///
/// ## Why not `super_editor_markdown`
///
/// Evaluated 2026-09-27 against pub.dev:
///
/// * `super_editor` 0.3.0-dev.52 (2026-06-11) is actively maintained on its
///   0.3.0 pre-release line (stable 0.2.7 dates from 2024 and pins
///   `http ^0.13`); it builds and runs on Flutter 3.47 for desktop and mobile
///   and lays out every paragraph in the direction of its first strong
///   character, so Arabic, English and mixed paragraphs each get their own
///   direction.
/// * `super_editor_markdown` 0.2.2 (2025-11-15) requires
///   `super_editor >=0.3.0-dev.31 <0.3.0-dev.40`, so it cannot be combined
///   with the current `super_editor`. Its serializer also rewrites markdown:
///   every unordered item becomes `* ` with two-space indents, every ordered
///   item `1.`, tasks are always `- [ ]` / `- [x]` (custom statuses such as
///   `[/]`, `[-]` and `[X]` are lost), paragraphs are re-joined with single
///   newlines (blank-line runs and `\r\n` are not kept), inline markup is
///   re-emitted from attributions, and it knows no frontmatter, wikilinks,
///   embeds, block IDs or Obsidian Tasks signifiers. It is not byte-exact.
///
/// ## What Strata does instead
///
/// The editor keeps the markdown **source**: every line of the body is one
/// `super_editor` paragraph holding that line's exact text, and the line
/// terminator that followed it is kept next to it. Serialising joins the
/// lines back with their own terminators, so a note that is loaded and saved
/// unchanged is byte-for-byte identical, and an edit of one paragraph
/// changes only that paragraph's bytes. Styling (headings, wikilinks,
/// embeds, tags, block IDs, task lines, code) comes from the Rust core's
/// editor hints (`CoreApi.editorHints`, UTF-16 spans), never from parsing in
/// Dart (PLAN L15). Because every construct stays raw source text, no
/// construct needs an atomic fallback node.
///
/// The frontmatter is excluded from the editor body: the core's
/// `HintKind.frontmatter` span marks it, and it is kept verbatim (always the
/// latest one the core streamed) in front of the body when saving.
library;

import 'package:flutter/foundation.dart' show immutable;
import 'package:strata_state/strata_state.dart' show EditorHint, HintKind;

/// A line of the body: its text and the terminator that followed it (`\n`,
/// `\r\n`, or `''` for the last line).
@immutable
final class SourceLine {
  /// Creates a source line.
  const new(this.text, this.eol);

  /// The line's text, without its terminator.
  final String text;

  /// The terminator that followed the line in the source.
  final String eol;

  @override
  bool operator ==(Object other) =>
      other is SourceLine && other.text == text && other.eol == eol;

  @override
  int get hashCode => Object.hash(text, eol);

  @override
  String toString() => 'SourceLine(${text.length}, ${eol.length})';
}

/// A note's markdown split into the frontmatter (kept verbatim, not edited)
/// and the body lines the editor edits.
@immutable
final class MarkdownSource {
  /// Creates a source from its parts.
  const new({
    required this.frontmatter,
    required this.lines,
    required this.newline,
  });

  /// Splits [content] into frontmatter and body lines. The frontmatter is
  /// the span of the core's [HintKind.frontmatter] hint in [hints] (none:
  /// the whole content is body).
  factory parse(String content, List<EditorHint> hints) {
    final end = frontmatterEnd(content, hints);
    final body = content.substring(end);
    final lines = splitLines(body);
    return MarkdownSource(
      frontmatter: content.substring(0, end),
      lines: lines,
      newline: lines.first.eol.isEmpty ? '\n' : lines.first.eol,
    );
  }

  /// The frontmatter block, verbatim (`''` when the note has none).
  final String frontmatter;

  /// The body lines, in order; never empty (an empty body is one empty
  /// line).
  final List<SourceLine> lines;

  /// The terminator for lines the user adds (the first terminator of the
  /// body as loaded, else `\n`).
  final String newline;

  /// The body markdown.
  String get body => joinLines(lines, newline);

  /// The whole markdown: frontmatter, then body.
  String get content => '$frontmatter$body';

  /// End of the frontmatter span among [hints], clamped to [content]
  /// (0 when there is none).
  static int frontmatterEnd(String content, List<EditorHint> hints) {
    for (final hint in hints) {
      if (hint.kind == HintKind.frontmatter && hint.start == 0) {
        return hint.end.clamp(0, content.length);
      }
    }
    return 0;
  }

  /// Splits [body] at `\n` (a preceding `\r` belongs to the terminator).
  /// The last line has an empty terminator; a body that ends with a
  /// terminator therefore ends with an empty line.
  static List<SourceLine> splitLines(String body) {
    final lines = <SourceLine>[];
    var start = 0;
    for (var i = 0; i < body.length; i++) {
      if (body.codeUnitAt(i) != 0x0A) continue;
      final crlf = i > start && body.codeUnitAt(i - 1) == 0x0D;
      lines.add(
        SourceLine(
          body.substring(start, crlf ? i - 1 : i),
          crlf ? '\r\n' : '\n',
        ),
      );
      start = i + 1;
    }
    lines.add(SourceLine(body.substring(start), ''));
    return lines;
  }

  /// Joins [lines] back into markdown. Every line but the last ends with
  /// its own terminator ([newline] when it has none, e.g. a line the user
  /// split off the last one); the last line never gets one.
  static String joinLines(List<SourceLine> lines, String newline) {
    final out = StringBuffer();
    for (var i = 0; i < lines.length; i++) {
      final line = lines[i];
      out.write(line.text);
      if (i < lines.length - 1) {
        out.write(line.eol.isEmpty ? newline : line.eol);
      }
    }
    return out.toString();
  }
}
