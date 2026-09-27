import 'package:flutter/widgets.dart';
import 'package:strata_home/src/generated/home_localizations.dart';
import 'package:strata_inbox/strata_inbox.dart';

export 'package:strata_home/src/generated/home_localizations.dart';

/// Access to the home feature strings.
extension HomeL10nContext on BuildContext {
  /// The [HomeLocalizations] of the closest [HomeL10nScope].
  HomeLocalizations get homeL10n => HomeLocalizations.of(this);
}

/// Makes the home (and the reused inbox and tasks) feature strings available
/// below it. A no-op when an ancestor already provides them.
class HomeL10nScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The subtree that reads `context.homeL10n`.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final existing = Localizations.of<HomeLocalizations>(
      context,
      HomeLocalizations,
    );
    if (existing != null) return InboxL10nScope(child: child);
    return Localizations.override(
      context: context,
      delegates: const [HomeLocalizations.delegate],
      child: InboxL10nScope(child: child),
    );
  }
}
