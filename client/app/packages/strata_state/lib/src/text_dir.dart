import 'package:flutter/widgets.dart';
import 'package:strata_bridge/strata_bridge.dart' show HighlightSpan, TextDir;

/// The [TextDirection] of a content direction the core computed (`*_dir`
/// fields, Unicode P2 first-strong rule): `null` for neutral text, which
/// then follows the ambient direction. A 1:1 adapter, no detection in Dart
/// (L15).
TextDirection? textDirectionOf(TextDir dir) => switch (dir) {
  TextDir.ltr => TextDirection.ltr,
  TextDir.rtl => TextDirection.rtl,
  TextDir.neutral => null,
};

/// The core's highlight spans (UTF-16) as [TextRange]s for
/// `StrataHighlightedText`. A 1:1 adapter.
List<TextRange> textRangesOf(List<HighlightSpan> spans) => [
  for (final span in spans) TextRange(start: span.start, end: span.end),
];
