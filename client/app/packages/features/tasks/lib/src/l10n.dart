import 'package:flutter/widgets.dart';
import 'package:strata_tasks/src/generated/tasks_localizations.dart';

export 'package:strata_tasks/src/generated/tasks_localizations.dart';

/// Access to the tasks feature strings.
extension TasksL10nContext on BuildContext {
  /// The [TasksLocalizations] of the closest [TasksL10nScope].
  TasksLocalizations get tasksL10n => TasksLocalizations.of(this);
}

/// Makes the tasks feature strings available below it (the app shell does
/// not need to register the delegate). A no-op when an ancestor already
/// provides them.
class TasksL10nScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The subtree that reads `context.tasksL10n`.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final existing = Localizations.of<TasksLocalizations>(
      context,
      TasksLocalizations,
    );
    if (existing != null) return child;
    return Localizations.override(
      context: context,
      delegates: const [TasksLocalizations.delegate],
      child: child,
    );
  }
}
