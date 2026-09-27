import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/src/theme/strata_theme.dart';

/// A keyboard shortcut hint (desktop), e.g. `⌘ K`, drawn as key caps in IBM
/// Plex Mono. Keys always read left-to-right.
class KeyboardHintChip extends StatelessWidget {
  /// Creates the hint.
  const new({required this.keys, super.key, this.onAccent = false});

  /// Key labels in press order, e.g. `['⌘', 'K']`.
  final List<String> keys;

  /// Styles the caps for use on an accent-filled button.
  final bool onAccent;

  /// The command key, drawn as an icon (no bundled font has the glyph).
  static const String commandKey = '⌘';

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final fg = onAccent ? colors.onAccent : colors.text2;
    final border = onAccent
        ? colors.onAccent.withValues(alpha: 0.6)
        : colors.border;
    final style = context.strataText.monoSmall.copyWith(color: fg, height: 1);
    return Semantics(
      label: context.l10n.shortcutSemantics(keys: keys.join(' ')),
      container: true,
      excludeSemantics: true,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        textDirection: TextDirection.ltr,
        children: [
          for (var i = 0; i < keys.length; i++) ...[
            if (i > 0) const SizedBox(width: 2),
            Container(
              constraints: const BoxConstraints(minWidth: 20, minHeight: 20),
              alignment: Alignment.center,
              padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
              decoration: BoxDecoration(
                border: Border.all(color: border),
                borderRadius: const BorderRadius.all(Radius.circular(4)),
                color: onAccent ? null : colors.surface,
              ),
              child: keys[i] == commandKey
                  ? Icon(Icons.keyboard_command_key, size: 12, color: fg)
                  : Text(
                      keys[i],
                      style: style,
                      textDirection: TextDirection.ltr,
                    ),
            ),
          ],
        ],
      ),
    );
  }
}
