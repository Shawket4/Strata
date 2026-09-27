import 'package:flutter/widgets.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';

export 'package:strata_directory/src/generated/directory_localizations.dart';

/// Access to the directory feature's strings.
extension DirectoryL10nContext on BuildContext {
  /// The [DirectoryLocalizations] of the ambient locale.
  DirectoryLocalizations get dirL10n => DirectoryLocalizations.of(this);
}

/// Adds the directory feature's localizations delegate below the app's
/// [Localizations] (same locale), so entry widgets work under any app shell.
class DirectoryLocalizationScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The feature UI.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (Localizations.of<DirectoryLocalizations>(
          context,
          DirectoryLocalizations,
        ) !=
        null) {
      return child;
    }
    return Localizations.override(
      context: context,
      delegates: const [DirectoryLocalizations.delegate],
      child: child,
    );
  }
}
