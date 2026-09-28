import 'package:flutter/material.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// [text] with the ranges the core matched (search highlights, UTF-16
/// offsets, in order) drawn as marks. Ranges outside the text are ignored;
/// nothing is matched in Dart.
class StrataHighlightedText extends StatelessWidget {
  /// Creates the text.
  const new(
    this.text, {
    required this.highlights,
    super.key,
    this.style,
    this.maxLines,
    this.textDirection,
  });

  /// The text.
  final String text;

  /// The highlighted ranges (start inclusive, end exclusive).
  final List<TextRange> highlights;

  /// Base style.
  final TextStyle? style;

  /// Maximum lines (ellipsized).
  final int? maxLines;

  /// The text's own direction (from the core's `*_dir`), if any.
  final TextDirection? textDirection;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final mark = TextStyle(
      backgroundColor: colors.warningTint,
      color: colors.text,
      fontWeight: FontWeight.w600,
    );
    final spans = <TextSpan>[];
    var at = 0;
    for (final range in highlights) {
      final start = range.start.clamp(at, text.length);
      final end = range.end.clamp(start, text.length);
      if (start > at) spans.add(TextSpan(text: text.substring(at, start)));
      if (end > start) {
        spans.add(TextSpan(text: text.substring(start, end), style: mark));
      }
      at = end;
    }
    if (at < text.length) spans.add(TextSpan(text: text.substring(at)));
    return Text.rich(
      TextSpan(children: spans),
      style: style,
      maxLines: maxLines,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
      textAlign: TextAlign.start,
      textDirection: textDirection,
    );
  }
}
