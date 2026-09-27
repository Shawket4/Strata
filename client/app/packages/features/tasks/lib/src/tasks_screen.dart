import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/async_body.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_tasks/src/recurrence_editor.dart';
import 'package:strata_tasks/src/task_detail.dart';
import 'package:strata_tasks/src/task_editor_sheet.dart';
import 'package:strata_tasks/src/task_row.dart';
import 'package:strata_ui/strata_ui.dart';

/// The list views of the Tasks screen on compact and medium, each one field
/// of the core's `TasksView` (no filtering in Dart).
enum TasksTab {
  /// `sections.today`.
  today,

  /// `sections.upcoming`.
  upcoming,

  /// `sections.overdue`.
  overdue,

  /// `sections.recurring`.
  recurring,

  /// `sections.noDate`.
  noDate,

  /// `done` (done and cancelled, most recent first).
  done,
}

/// The views of the expanded Tasks table.
enum TasksTableTab {
  /// Overdue, Today, Upcoming and No date, each as the core groups them.
  open,

  /// `sections.recurring`.
  recurring,

  /// `done`.
  done,
}

/// The Tasks destination (medium and expanded; on compact tasks also live on
/// Home, D28): task lists with complete / reopen checkboxes, recurrence
/// summary and reminder bell, the task detail, the new-task sheet and, on
/// desktop layouts, keyboard shortcuts (J/K move, X done, R repeat, D due
/// date, T new task, Enter open in note).
///
/// * compact: one list with view tabs; a task opens full-screen
///   ([onOpenTask], or a pushed [TaskDetailScreen]);
/// * medium: list + detail pane;
/// * expanded: task table + detail panel.
class TasksScreen extends HookConsumerWidget {
  /// Creates the Tasks screen.
  const new({
    super.key,
    this.initialTaskId,
    this.initialTab = TasksTab.today,
    this.onOpenTask,
    this.onOpenNote,
    this.onOpenEntity,
    this.onOpenExisting,
  });

  /// The icon that represents this feature.
  static const IconData icon = Icons.check_circle_outline;

  /// The task selected in the detail pane when the screen opens (deep link).
  final String? initialTaskId;

  /// The list view shown first on compact and medium.
  final TasksTab initialTab;

  /// Opens a task full-screen on compact (the app shell's route); when
  /// `null` a [TaskDetailScreen] is pushed.
  final ValueChanged<String>? onOpenTask;

  /// Opens a note by id (a task's home note).
  final ValueChanged<String>? onOpenNote;

  /// Opens a linked entity by id.
  final ValueChanged<String>? onOpenEntity;

  /// Opens an existing item offered by the duplicate check of New task.
  final ValueChanged<CandidateItem>? onOpenExisting;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final value = ref.watch(tasksProvider);
    final selected = useState<String?>(initialTaskId);
    final tab = useState(initialTab);
    final tableTab = useState(TasksTableTab.open);
    final sizeClass = SizeClass.of(context);
    return TasksL10nScope(
      child: Builder(
        builder: (context) => CoreAsyncBody(
          value: value,
          errorTitle: context.tasksL10n.tasksLoadError,
          data: (view) {
            void openCompact(String id) {
              final open = onOpenTask;
              if (open != null) {
                open(id);
                return;
              }
              Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => TaskDetailScreen(
                    taskId: id,
                    onOpenNote: onOpenNote,
                    onOpenEntity: onOpenEntity,
                  ),
                ),
              );
            }

            void newTask() => unawaited(
              TaskEditorSheet.show(
                context,
                onOpenExisting: onOpenExisting,
                onCreated: sizeClass == SizeClass.compact
                    ? null
                    : (id) => selected.value = id,
              ),
            );

            final selectedId = selected.value;
            final detail = selectedId == null
                ? StrataEmptyState(
                    icon: Icons.task_alt,
                    title: context.tasksL10n.tasksSelectPrompt,
                  )
                : TaskDetailPane(
                    key: ValueKey(selectedId),
                    taskId: selectedId,
                    onOpenNote: onOpenNote,
                    onOpenEntity: onOpenEntity,
                    onClose: sizeClass == SizeClass.expanded
                        ? () => selected.value = null
                        : null,
                  );
            switch (sizeClass) {
              case SizeClass.compact:
                return _TabbedList(
                  view: view,
                  tab: tab.value,
                  onTab: (t) => tab.value = t,
                  onOpen: openCompact,
                  onNewTask: newTask,
                );
              case SizeClass.medium:
                final rows = _rowsForTab(view, tab.value);
                return _TaskKeyboard(
                  rows: rows,
                  selected: selectedId,
                  onSelect: (id) => selected.value = id,
                  onNewTask: newTask,
                  onOpenNote: onOpenNote,
                  child: StrataPanes(
                    list: _TabbedList(
                      view: view,
                      tab: tab.value,
                      onTab: (t) => tab.value = t,
                      onOpen: (id) => selected.value = id,
                      onNewTask: newTask,
                      selected: selectedId,
                    ),
                    detail: detail,
                  ),
                );
              case SizeClass.expanded:
                final rows = _rowsForTable(view, tableTab.value);
                return _TaskKeyboard(
                  rows: rows,
                  selected: selectedId,
                  onSelect: (id) => selected.value = id,
                  onNewTask: newTask,
                  onOpenNote: onOpenNote,
                  child: Row(
                    children: [
                      Expanded(
                        child: _TaskTable(
                          view: view,
                          tab: tableTab.value,
                          onTab: (t) => tableTab.value = t,
                          selected: selectedId,
                          onSelect: (id) => selected.value = id,
                          onNewTask: newTask,
                        ),
                      ),
                      VerticalDivider(
                        width: 1,
                        color: context.strataColors.border,
                      ),
                      SizedBox(
                        width: StrataLayout.contextPanelWidth,
                        child: Semantics(
                          container: true,
                          label: context.tasksL10n.tasksDetailPanelLabel,
                          child: ColoredBox(
                            color: context.strataColors.surface,
                            child: detail,
                          ),
                        ),
                      ),
                    ],
                  ),
                );
            }
          },
        ),
      ),
    );
  }
}

/// The tasks of a list view, in the order they are rendered.
List<TaskItem> _rowsForTab(TasksView view, TasksTab tab) => switch (tab) {
  TasksTab.today => view.sections.today,
  TasksTab.upcoming => view.sections.upcoming,
  TasksTab.overdue => view.sections.overdue,
  TasksTab.recurring => view.sections.recurring,
  TasksTab.noDate => view.sections.noDate,
  TasksTab.done => view.done,
};

/// The table's groups, in the order they are rendered.
List<(String Function(TasksLocalizations), List<TaskItem>, bool)> _groups(
  TasksView view,
  TasksTableTab tab,
) => switch (tab) {
  TasksTableTab.open => [
    ((l) => l.tasksTabOverdue, view.sections.overdue, true),
    ((l) => l.tasksTabToday, view.sections.today, false),
    ((l) => l.tasksTabUpcoming, view.sections.upcoming, false),
    ((l) => l.tasksTabNoDate, view.sections.noDate, false),
  ],
  TasksTableTab.recurring => [
    ((l) => l.tasksTabRecurring, view.sections.recurring, false),
  ],
  TasksTableTab.done => [((l) => l.tasksTabDone, view.done, false)],
};

List<TaskItem> _rowsForTable(TasksView view, TasksTableTab tab) => [
  for (final (_, tasks, _) in _groups(view, tab)) ...tasks,
];

/// Desktop keyboard shortcuts over the rendered rows.
class _TaskKeyboard extends ConsumerWidget {
  const new({
    required this.rows,
    required this.selected,
    required this.onSelect,
    required this.onNewTask,
    required this.onOpenNote,
    required this.child,
  });

  final List<TaskItem> rows;
  final String? selected;
  final ValueChanged<String> onSelect;
  final VoidCallback onNewTask;
  final ValueChanged<String>? onOpenNote;
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final index = rows.indexWhere((t) => t.id == selected);
    final current = index < 0 ? null : rows[index];

    void move(int delta) {
      if (rows.isEmpty) return;
      final next = index < 0 ? 0 : (index + delta).clamp(0, rows.length - 1);
      onSelect(rows[next].id);
    }

    void toggle() {
      final task = current;
      if (task == null) return;
      final api = ref.read(coreApiProvider);
      unawaited(
        forwardIntent(
          context,
          task.state == TaskState.open
              ? api.completeTask(taskId: task.id)
              : api.reopenTask(taskId: task.id),
        ),
      );
    }

    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.keyJ): () => move(1),
        const SingleActivator(LogicalKeyboardKey.keyK): () => move(-1),
        const SingleActivator(LogicalKeyboardKey.keyX): toggle,
        const SingleActivator(LogicalKeyboardKey.keyR): () {
          final task = current;
          if (task != null) {
            unawaited(RecurrenceEditor.show(context, task: task));
          }
        },
        const SingleActivator(LogicalKeyboardKey.keyD): () {
          final task = current;
          if (task != null) unawaited(pickTaskDue(context, ref, task));
        },
        const SingleActivator(LogicalKeyboardKey.keyT): onNewTask,
        const SingleActivator(LogicalKeyboardKey.enter): () {
          final task = current;
          final open = onOpenNote;
          if (task != null && open != null) open(task.noteId);
        },
      },
      child: Focus(autofocus: true, includeSemantics: false, child: child),
    );
  }
}

class _TabbedList extends StatelessWidget {
  const new({
    required this.view,
    required this.tab,
    required this.onTab,
    required this.onOpen,
    required this.onNewTask,
    this.selected,
  });

  final TasksView view;
  final TasksTab tab;
  final ValueChanged<TasksTab> onTab;
  final ValueChanged<String> onOpen;
  final VoidCallback onNewTask;
  final String? selected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final text = context.strataText;
    final rows = _rowsForTab(view, tab);
    String label(TasksTab t) => switch (t) {
      TasksTab.today => l10n.tasksTabToday,
      TasksTab.upcoming => l10n.tasksTabUpcoming,
      TasksTab.overdue => l10n.tasksTabOverdue,
      TasksTab.recurring => l10n.tasksTabRecurring,
      TasksTab.noDate => l10n.tasksTabNoDate,
      TasksTab.done => l10n.tasksTabDone,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s4,
            StrataSpacing.s3,
            StrataSpacing.s3,
            StrataSpacing.s1,
          ),
          child: Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: StrataSpacing.s3,
            runSpacing: StrataSpacing.s2,
            children: [
              Semantics(
                header: true,
                container: true,
                child: Text(l10n.tasksTitle, style: text.title),
              ),
              FilledButton.icon(
                style: tallFilledButton,
                onPressed: onNewTask,
                icon: const Icon(Icons.add),
                label: Text(
                  l10n.tasksNewTask,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
          child: Row(
            children: [
              for (final t in TasksTab.values)
                Padding(
                  padding: const EdgeInsetsDirectional.only(
                    end: StrataSpacing.s2,
                  ),
                  child: ChoiceChip(
                    label: Text(
                      t == TasksTab.done
                          ? label(t)
                          : l10n.tasksTabWithCount(
                              label: label(t),
                              count: _rowsForTab(view, t).length,
                            ),
                    ),
                    selected: t == tab,
                    onSelected: (_) => onTab(t),
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: StrataSpacing.s1),
        Expanded(
          child: rows.isEmpty
              ? StrataEmptyState(
                  icon: Icons.task_alt,
                  title: l10n.tasksEmptyTitle,
                  message: l10n.tasksEmptyMessage,
                )
              : ListView(
                  children: [
                    for (final task in rows)
                      if (tab == TasksTab.recurring)
                        RecurringRuleRow(task: task, onOpen: onOpen)
                      else
                        TaskRow(
                          task: task,
                          overdue: tab == TasksTab.overdue,
                          selected: task.id == selected,
                          onOpen: onOpen,
                        ),
                  ],
                ),
        ),
      ],
    );
  }
}

class _TaskTable extends StatelessWidget {
  const new({
    required this.view,
    required this.tab,
    required this.onTab,
    required this.selected,
    required this.onSelect,
    required this.onNewTask,
  });

  final TasksView view;
  final TasksTableTab tab;
  final ValueChanged<TasksTableTab> onTab;
  final String? selected;
  final ValueChanged<String> onSelect;
  final VoidCallback onNewTask;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final groups = [
      for (final group in _groups(view, tab))
        if (group.$2.isNotEmpty) group,
    ];
    String label(TasksTableTab t) => switch (t) {
      TasksTableTab.open => l10n.tasksTabOpen,
      TasksTableTab.recurring => l10n.tasksTabWithCount(
        label: l10n.tasksTabRecurring,
        count: view.sections.recurring.length,
      ),
      TasksTableTab.done => l10n.tasksTabDone,
    };
    final header = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2, letterSpacing: 0.4);
    return ListView(
      padding: const EdgeInsets.all(StrataSpacing.s6),
      children: [
        Wrap(
          spacing: StrataSpacing.s4,
          runSpacing: StrataSpacing.s3,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Semantics(
              header: true,
              container: true,
              child: Text(l10n.tasksTitle, style: text.display),
            ),
            SegmentedButton<TasksTableTab>(
              segments: [
                for (final t in TasksTableTab.values)
                  ButtonSegment(value: t, label: Text(label(t))),
              ],
              selected: {tab},
              showSelectedIcon: false,
              onSelectionChanged: (s) => onTab(s.first),
            ),
            FilledButton.icon(
              style: tallFilledButton,
              onPressed: onNewTask,
              icon: const Icon(Icons.add),
              label: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(l10n.tasksNewTask),
                  const SizedBox(width: StrataSpacing.s2),
                  const KeyboardHintChip(keys: ['T'], onAccent: true),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: StrataSpacing.s3),
        Semantics(
          container: true,
          label: l10n.tasksShortcutsLabel,
          child: Wrap(
            spacing: StrataSpacing.s4,
            runSpacing: StrataSpacing.s1,
            children: [
              _Hint(keys: const ['J', 'K'], label: l10n.tasksShortcutMove),
              _Hint(keys: const ['X'], label: l10n.tasksShortcutDone),
              _Hint(keys: const ['R'], label: l10n.tasksShortcutRepeat),
              _Hint(keys: const ['D'], label: l10n.tasksShortcutDue),
              _Hint(keys: const ['Enter'], label: l10n.tasksShortcutOpen),
            ],
          ),
        ),
        const SizedBox(height: StrataSpacing.s4),
        if (groups.isEmpty)
          StrataEmptyState(
            icon: Icons.task_alt,
            title: l10n.tasksEmptyTitle,
            message: l10n.tasksEmptyMessage,
          )
        else
          DecoratedBox(
            decoration: BoxDecoration(
              color: colors.surface,
              border: Border.all(color: colors.border),
              borderRadius: StrataRadii.cardRadius,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s3,
                    vertical: StrataSpacing.s2,
                  ),
                  child: Row(
                    children: [
                      const SizedBox(width: 48),
                      Expanded(
                        flex: 4,
                        child: Text(l10n.tasksColumnTask, style: header),
                      ),
                      Expanded(
                        flex: 2,
                        child: Text(l10n.tasksColumnLinked, style: header),
                      ),
                      Expanded(
                        flex: 2,
                        child: Text(l10n.tasksColumnRepeat, style: header),
                      ),
                      Expanded(
                        flex: 2,
                        child: Text(l10n.tasksColumnRemind, style: header),
                      ),
                      Expanded(
                        flex: 2,
                        child: Text(l10n.tasksColumnDue, style: header),
                      ),
                    ],
                  ),
                ),
                for (final (title, tasks, overdue) in groups) ...[
                  Container(
                    color: colors.surface2,
                    padding: const EdgeInsets.symmetric(
                      horizontal: StrataSpacing.s4,
                      vertical: StrataSpacing.s1,
                    ),
                    child: Semantics(
                      header: true,
                      container: true,
                      child: Text(
                        title(l10n),
                        style: header.copyWith(
                          color: overdue ? colors.dangerText : colors.text2,
                        ),
                      ),
                    ),
                  ),
                  for (final task in tasks)
                    _TableRow(
                      task: task,
                      overdue: overdue,
                      selected: task.id == selected,
                      onSelect: onSelect,
                    ),
                ],
              ],
            ),
          ),
        const SizedBox(height: StrataSpacing.s4),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.info_outline, size: 16, color: colors.text2),
            const SizedBox(width: StrataSpacing.s2),
            Expanded(
              child: Text(
                l10n.tasksFooterNote,
                style: text.caption.copyWith(color: colors.text2),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _Hint extends StatelessWidget {
  const new({required this.keys, required this.label});

  final List<String> keys;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        KeyboardHintChip(keys: keys),
        const SizedBox(width: StrataSpacing.s1),
        ExcludeSemantics(
          child: Text(
            label,
            style: context.strataText.caption.copyWith(
              color: context.strataColors.text2,
            ),
          ),
        ),
      ],
    );
  }
}

class _TableRow extends StatelessWidget {
  const new({
    required this.task,
    required this.overdue,
    required this.selected,
    required this.onSelect,
  });

  final TaskItem task;
  final bool overdue;
  final bool selected;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final cell = text.bodySmall.copyWith(color: colors.text2);
    final due = task.due;
    final recurrence = task.recurrence;
    final closed = task.state != TaskState.open;
    return Material(
      color: selected ? colors.accentTint : Colors.transparent,
      child: InkWell(
        onTap: () => onSelect(task.id),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s3),
          child: Row(
            children: [
              SizedBox(width: 48, child: TaskCheckbox(task: task)),
              Expanded(
                flex: 4,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    vertical: StrataSpacing.s2,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        task.description,
                        style: text.bodySmall
                            .withWeight(FontWeight.w600)
                            .copyWith(
                              decoration: closed
                                  ? TextDecoration.lineThrough
                                  : null,
                            ),
                      ),
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              task.noteTitle,
                              style: text.caption.copyWith(color: colors.text2),
                            ),
                          ),
                          if (task.pendingSync) ...[
                            const SizedBox(width: StrataSpacing.s1),
                            const NotSyncedMarker(),
                          ],
                        ],
                      ),
                      if (!task.recurrenceUnderstood)
                        StatusPill(
                          label: l10n.tasksRecurrenceNotUnderstood,
                          tone: StatusTone.warning,
                        ),
                    ],
                  ),
                ),
              ),
              Expanded(
                flex: 2,
                child: task.links.isEmpty
                    ? Text(l10n.commonNone, style: cell)
                    : Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          for (final link in task.links)
                            Text(link.title, style: cell),
                        ],
                      ),
              ),
              Expanded(
                flex: 2,
                child: recurrence == null
                    ? Text(l10n.commonNone, style: cell)
                    : Row(
                        children: [
                          Icon(Icons.repeat, size: 14, color: colors.text2),
                          const SizedBox(width: 2),
                          Flexible(child: Text(recurrence, style: cell)),
                        ],
                      ),
              ),
              Expanded(
                flex: 2,
                child: task.reminders.isEmpty
                    ? Text(l10n.commonNone, style: cell)
                    : TaskReminderBell(reminders: task.reminders),
              ),
              Expanded(
                flex: 2,
                child: Text(
                  due == null
                      ? l10n.commonNone
                      : overdue
                      ? l10n.tasksOverdueSince(date: due)
                      : l10n.tasksDateShort(date: due),
                  style: overdue
                      ? cell
                            .withWeight(FontWeight.w600)
                            .copyWith(color: colors.dangerText)
                      : cell,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
