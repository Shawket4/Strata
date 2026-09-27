import 'package:flutter/widgets.dart';
import 'package:strata_maps/src/generated/maps_localizations.dart';

export 'package:strata_maps/src/generated/maps_localizations.dart';

/// Access to the maps feature's strings.
extension MapsL10nContext on BuildContext {
  /// The [MapsLocalizations] of the ambient locale.
  MapsLocalizations get mapsL10n => MapsLocalizations.of(this);
}

/// Adds the maps feature's localizations delegate below the app's
/// [Localizations] (same locale), so entry widgets work under any app shell.
class MapsLocalizationScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The feature UI.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (Localizations.of<MapsLocalizations>(context, MapsLocalizations) !=
        null) {
      return child;
    }
    return Localizations.override(
      context: context,
      delegates: const [MapsLocalizations.delegate],
      child: child,
    );
  }
}
