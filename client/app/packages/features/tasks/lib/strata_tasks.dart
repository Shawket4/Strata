/// Strata tasks feature (UI only; view-models come from the Rust core,
/// PLAN L15): the Tasks destination, task detail, recurrence editor, new-task
/// sheet and the shared "Already exists" duplicate prompt.
library;

export 'src/async_body.dart';
export 'src/duplicate_prompt.dart';
export 'src/l10n.dart'
    show
        TasksL10nContext,
        TasksL10nScope,
        TasksLocalizations,
        lookupTasksLocalizations;
export 'src/labels.dart';
export 'src/recurrence_editor.dart';
export 'src/task_detail.dart';
export 'src/task_editor_sheet.dart';
export 'src/task_row.dart';
export 'src/tasks_screen.dart';
