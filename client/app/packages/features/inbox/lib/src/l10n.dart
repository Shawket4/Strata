import 'package:flutter/widgets.dart';
import 'package:strata_inbox/src/generated/inbox_localizations.dart';
import 'package:strata_tasks/strata_tasks.dart';

export 'package:strata_inbox/src/generated/inbox_localizations.dart';

/// Access to the inbox feature strings.
extension InboxL10nContext on BuildContext {
  /// The [InboxLocalizations] of the closest [InboxL10nScope].
  InboxLocalizations get inboxL10n => InboxLocalizations.of(this);
}

/// Makes the inbox (and shared tasks) feature strings available below it.
/// A no-op when an ancestor already provides them.
class InboxL10nScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The subtree that reads `context.inboxL10n`.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final existing = Localizations.of<InboxLocalizations>(
      context,
      InboxLocalizations,
    );
    if (existing != null) return TasksL10nScope(child: child);
    return Localizations.override(
      context: context,
      delegates: const [InboxLocalizations.delegate],
      child: TasksL10nScope(child: child),
    );
  }
}
