import 'package:flutter/widgets.dart';
import 'package:strata_ask/src/generated/ask_localizations.dart';

export 'package:strata_ask/src/generated/ask_localizations.dart';

/// Access to the ask feature's strings.
extension AskL10nContext on BuildContext {
  /// The [AskLocalizations] of the ambient locale.
  AskLocalizations get askL10n => AskLocalizations.of(this);
}

/// Adds the ask feature's localizations delegate below the app's
/// [Localizations] (same locale), so entry widgets work under any app shell.
class AskLocalizationScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The feature UI.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (Localizations.of<AskLocalizations>(context, AskLocalizations) != null) {
      return child;
    }
    return Localizations.override(
      context: context,
      delegates: const [AskLocalizations.delegate],
      child: child,
    );
  }
}
