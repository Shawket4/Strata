import 'package:flutter/material.dart';
import 'package:strata_ui/src/brand/strata_symbol.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// The Strata lockup: the symbol in tide followed by the lowercase
/// "strata" wordmark in Quicksand 700 in the text colour.
///
/// The Latin lockup keeps its left-to-right order in an Arabic UI.
class StrataWordmark extends StatelessWidget {
  /// Creates the lockup with the wordmark set at [fontSize].
  const new({
    super.key,
    this.fontSize = 22,
    this.showSymbol = true,
    this.semanticLabel = 'Strata',
  });

  /// Wordmark size; the symbol is drawn at the same size.
  final double fontSize;

  /// Whether to draw the symbol before the wordmark.
  final bool showSymbol;

  /// Accessibility label of the whole lockup.
  final String semanticLabel;

  /// The wordmark text (lowercase by design).
  static const String text = 'strata';

  @override
  Widget build(BuildContext context) {
    final style = context.strataText.wordmark.copyWith(fontSize: fontSize);
    return Semantics(
      label: semanticLabel,
      container: true,
      child: ExcludeSemantics(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          textDirection: TextDirection.ltr,
          children: [
            if (showSymbol) ...[
              StrataSymbol(size: fontSize * 1.1),
              SizedBox(width: fontSize * 0.35),
            ],
            Text(
              text,
              style: style,
              textDirection: TextDirection.ltr,
              textScaler: TextScaler.noScaling,
            ),
          ],
        ),
      ),
    );
  }
}
