import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/async_body.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_tasks/src/recurrence_editor.dart';
import 'package:strata_tasks/src/task_row.dart';
import 'package:strata_ui/strata_ui.dart';

/// Picks a new due date and sends it with `CoreApi.updateTask`
/// (`TaskPatch.due`, a date as the core stores it: UTC midnight).
Future<void> pickTaskDue(
  BuildContext context,
  WidgetRef ref,
  TaskItem task,
) async {
  final api = ref.read(coreApiProvider);
  final picked = await showDatePicker(
    context: context,
    initialDate: task.due,
    firstDate: DateTime(2000),
    lastDate: DateTime(2100),
  );
  if (picked == null || !context.mounted) return;
  await forwardIntent(
    context,
    api.updateTask(
      taskId: task.id,
      patch: TaskPatch(
        due: DateTime.utc(picked.year, picked.month, picked.day),
        clearDue: false,
        clearRecurrence: false,
      ),
    ),
  );
}

/// Asks for a reminder's day and time and adds it (`add_reminder`; a
/// wall-clock time, sent as the core stores it: a UTC-encoded local time).
Future<void> addTaskReminder(
  BuildContext context,
  WidgetRef ref,
  TaskItem task,
) async {
  final api = ref.read(coreApiProvider);
  final day = await showDatePicker(
    context: context,
    initialDate: task.due,
    firstDate: DateTime(2000),
    lastDate: DateTime(2100),
  );
  if (day == null || !context.mounted) return;
  final time = await showTimePicker(
    context: context,
    initialTime: const TimeOfDay(hour: 9, minute: 0),
  );
  if (time == null || !context.mounted) return;
  await forwardIntent(
    context,
    api.addReminder(
      taskId: task.id,
      at: DateTime.utc(day.year, day.month, day.day, time.hour, time.minute),
    ),
  );
}

/// Task detail as a full screen (compact, or a deep link): text, due, repeat
/// rule, reminders, linked entities, home note, completed-occurrence history
/// and Mark done / Cancel task (occurrences are never skipped).
class TaskDetailScreen extends StatelessWidget {
  /// Creates the screen for [taskId].
  const new({
    required this.taskId,
    super.key,
    this.onOpenNote,
    this.onOpenEntity,
  });

  /// The task's block ID (`t-…`).
  final String taskId;

  /// Opens a note by id (the task's home note).
  final ValueChanged<String>? onOpenNote;

  /// Opens a linked entity by id.
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context) {
    return TasksL10nScope(
      child: Builder(
        builder: (context) => Scaffold(
          appBar: AppBar(title: Text(context.tasksL10n.tasksDetailTitle)),
          body: TaskDetailPane(
            taskId: taskId,
            onOpenNote: onOpenNote,
            onOpenEntity: onOpenEntity,
          ),
        ),
      ),
    );
  }
}

/// The task detail content, used as the detail pane of the Tasks screen and
/// inside [TaskDetailScreen].
class TaskDetailPane extends ConsumerWidget {
  /// Creates the pane for [taskId].
  const new({
    required this.taskId,
    super.key,
    this.onOpenNote,
    this.onOpenEntity,
    this.onClose,
  });

  /// The task's block ID.
  final String taskId;

  /// Opens a note by id.
  final ValueChanged<String>? onOpenNote;

  /// Opens a linked entity by id.
  final ValueChanged<String>? onOpenEntity;

  /// Closes the pane (expanded context panel).
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final value = ref.watch(taskProvider(taskId));
    return TasksL10nScope(
      child: Builder(
        builder: (context) => CoreAsyncBody(
          value: value,
          errorTitle: context.tasksL10n.tasksDetailLoadError,
          data: (screen) {
            final task = screen.task;
            if (task == null) {
              return StrataEmptyState(
                icon: Icons.search_off,
                title: context.tasksL10n.tasksNotFoundTitle,
                message: context.tasksL10n.tasksNotFoundMessage,
              );
            }
            return _TaskDetailBody(
              screen: screen,
              task: task,
              onOpenNote: onOpenNote,
              onOpenEntity: onOpenEntity,
              onClose: onClose,
            );
          },
        ),
      ),
    );
  }
}

class _TaskDetailBody extends ConsumerWidget {
  const new({
    required this.screen,
    required this.task,
    required this.onOpenNote,
    required this.onOpenEntity,
    required this.onClose,
  });

  final TaskScreen screen;
  final TaskItem task;
  final ValueChanged<String>? onOpenNote;
  final ValueChanged<String>? onOpenEntity;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final api = ref.read(coreApiProvider);
    final open = task.state == TaskState.open;
    final due = task.dueLabel;
    final late = task.latenessLabel;
    final recurrence = task.recurrence;
    final openNote = onOpenNote;
    final close = onClose;
    final next = screen.nextOccurrenceLabel;
    final delivery = screen.deliveryLabel;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      screen.locationLabel.isEmpty
                          ? task.noteTitle
                          : screen.locationLabel,
                      style: text.monoSmall.copyWith(color: colors.text2),
                    ),
                  ),
                  if (openNote != null)
                    IconButton(
                      tooltip: l10n.tasksActionOpenInNote,
                      onPressed: () => openNote(task.noteId),
                      icon: const Icon(Icons.open_in_new),
                    ),
                  if (close != null)
                    IconButton(
                      tooltip: l10n.tasksActionCloseDetail,
                      onPressed: close,
                      icon: const Icon(Icons.close),
                    ),
                ],
              ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  TaskCheckbox(task: task),
                  const SizedBox(width: StrataSpacing.s1),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.only(top: StrataSpacing.s2),
                      child: Semantics(
                        header: true,
                        container: true,
                        child: Text(
                          task.description,
                          textDirection: textDirectionOf(task.descriptionDir),
                          textAlign: TextAlign.start,
                          style: text.title,
                        ),
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: l10n.tasksActionEditText,
                    onPressed: () => _editText(context, ref),
                    icon: const Icon(Icons.edit_outlined),
                  ),
                ],
              ),
              const SizedBox(height: StrataSpacing.s2),
              Wrap(
                spacing: StrataSpacing.s2,
                runSpacing: StrataSpacing.s1,
                children: [
                  StatusPill(
                    label: recurrence == null
                        ? l10n.tasksOneOff
                        : l10n.tasksRecurring,
                    tone: recurrence == null
                        ? StatusTone.neutral
                        : StatusTone.info,
                    icon: recurrence == null ? Icons.event : Icons.repeat,
                  ),
                  if (task.state == TaskState.done)
                    StatusPill(
                      label: l10n.tasksStateDone,
                      tone: StatusTone.success,
                    ),
                  if (task.state == TaskState.cancelled)
                    StatusPill(label: l10n.tasksCancelled),
                  if (task.pendingSync)
                    StatusPill(
                      label: l10n.commonNotSynced,
                      icon: Icons.cloud_upload_outlined,
                    ),
                ],
              ),
              const SizedBox(height: StrataSpacing.s4),
              _Field(
                icon: Icons.repeat,
                label: l10n.tasksFieldRepeat,
                value: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      recurrence ?? l10n.tasksDoesNotRepeat,
                      style: text.bodySmall.withWeight(FontWeight.w500),
                    ),
                    if (recurrence != null && next != null)
                      Text(
                        next,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    if (!task.recurrenceUnderstood)
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: StatusPill(
                          label: l10n.tasksRecurrenceNotUnderstood,
                          tone: StatusTone.warning,
                        ),
                      ),
                  ],
                ),
                action: TextButton(
                  onPressed: () => RecurrenceEditor.show(
                    context,
                    task: task,
                    line: screen.line,
                  ),
                  child: Text(
                    l10n.tasksActionEditRule,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              _Field(
                icon: Icons.event,
                label: l10n.tasksFieldDue,
                value: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      due ?? l10n.tasksNoDueDate,
                      style: text.bodySmall.withWeight(FontWeight.w500),
                    ),
                    if (late != null && open)
                      Text(
                        late,
                        style: text.caption
                            .withWeight(FontWeight.w600)
                            .copyWith(color: colors.dangerText),
                      )
                    else if (task.nextInLabel case final inLabel? when open)
                      Text(
                        inLabel,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                  ],
                ),
                action: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    IconButton(
                      tooltip: l10n.tasksActionChangeDue,
                      onPressed: () => pickTaskDue(context, ref, task),
                      icon: const Icon(Icons.edit_calendar_outlined),
                    ),
                    if (due != null)
                      IconButton(
                        tooltip: l10n.tasksActionClearDue,
                        onPressed: () => unawaited(
                          forwardIntent(
                            context,
                            api.updateTask(
                              taskId: task.id,
                              patch: const TaskPatch(
                                clearDue: true,
                                clearRecurrence: false,
                              ),
                            ),
                          ),
                        ),
                        icon: const Icon(Icons.event_busy_outlined),
                      ),
                  ],
                ),
              ),
              _Field(
                icon: Icons.notifications_none_rounded,
                label: l10n.tasksFieldReminders,
                value: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (task.reminders.isEmpty)
                      Text(l10n.tasksNoReminders, style: text.bodySmall),
                    for (final reminder in task.reminders)
                      Row(
                        children: [
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  reminder.timeLabel,
                                  style: text.bodySmall.withWeight(
                                    FontWeight.w500,
                                  ),
                                ),
                                if (reminder.offsetLabel.isNotEmpty)
                                  Text(
                                    reminder.offsetLabel,
                                    style: text.caption.copyWith(
                                      color: colors.text2,
                                    ),
                                  ),
                              ],
                            ),
                          ),
                          IconButton(
                            tooltip: l10n.editorRemoveReminder(
                              time: reminder.timeLabel,
                            ),
                            onPressed: () => unawaited(
                              forwardIntent(
                                context,
                                api.removeReminder(
                                  taskId: task.id,
                                  at: reminder.localAt,
                                ),
                              ),
                            ),
                            icon: const Icon(Icons.close, size: 18),
                          ),
                        ],
                      ),
                    if (delivery != null && task.reminders.isNotEmpty)
                      Text(
                        delivery,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                  ],
                ),
                action: TextButton.icon(
                  onPressed: () =>
                      unawaited(addTaskReminder(context, ref, task)),
                  icon: const Icon(Icons.add),
                  label: Text(
                    l10n.tasksActionAddReminder,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              _Field(
                icon: Icons.link,
                label: l10n.tasksFieldLinked,
                value: task.links.isEmpty
                    ? Text(l10n.tasksNoLinks, style: text.bodySmall)
                    : Wrap(
                        spacing: StrataSpacing.s1,
                        runSpacing: StrataSpacing.s1,
                        children: [
                          for (final link in task.links)
                            EntityRefChip(entity: link, onOpen: onOpenEntity),
                        ],
                      ),
              ),
              _Field(
                icon: Icons.description_outlined,
                label: l10n.tasksFieldHomeNote,
                value: Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: TextButton(
                    onPressed: openNote == null
                        ? null
                        : () => openNote(task.noteId),
                    child: Text(
                      task.noteTitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ),
              ),
              _Field(
                icon: Icons.code,
                label: l10n.tasksFieldStoredLine,
                value: Text(
                  screen.line,
                  textDirection: TextDirection.ltr,
                  style: text.monoSmall.copyWith(color: colors.text2),
                ),
              ),
              const SizedBox(height: StrataSpacing.s4),
              Align(
                alignment: AlignmentDirectional.centerStart,
                child: Semantics(
                  header: true,
                  container: true,
                  child: Text(l10n.tasksHistoryTitle, style: text.titleSmall),
                ),
              ),
              Text(
                l10n.tasksHistorySubtitle,
                style: text.caption.copyWith(color: colors.text2),
              ),
              const SizedBox(height: StrataSpacing.s2),
              if (screen.history.isEmpty)
                Text(
                  l10n.tasksHistoryEmpty,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              for (final occurrence in screen.history)
                _HistoryRow(occurrence: occurrence),
              if (recurrence != null) ...[
                const SizedBox(height: StrataSpacing.s4),
                Container(
                  padding: const EdgeInsets.all(StrataSpacing.s3),
                  decoration: BoxDecoration(
                    color: colors.infoTint,
                    borderRadius: StrataRadii.cardRadius,
                  ),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(
                        Icons.info_outline,
                        size: 18,
                        color: colors.infoText,
                      ),
                      const SizedBox(width: StrataSpacing.s2),
                      Expanded(
                        child: Text(
                          l10n.tasksNoSkipping,
                          style: text.bodySmall.copyWith(
                            color: colors.infoText,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              const SizedBox(height: StrataSpacing.s5),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: StrataSpacing.s2,
                runSpacing: StrataSpacing.s2,
                children: open
                    ? [
                        OutlinedButton(
                          onPressed: () => unawaited(
                            forwardIntent(
                              context,
                              api.cancelTask(taskId: task.id),
                            ),
                          ),
                          child: Text(
                            l10n.tasksActionCancelTask,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        FilledButton.icon(
                          style: tallFilledButton,
                          onPressed: () => unawaited(
                            forwardIntent(
                              context,
                              api.completeTask(taskId: task.id),
                            ),
                          ),
                          icon: const Icon(Icons.check),
                          label: Text(
                            l10n.tasksActionMarkDone,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ]
                    : [
                        FilledButton.icon(
                          style: tallFilledButton,
                          onPressed: () => unawaited(
                            forwardIntent(
                              context,
                              api.reopenTask(taskId: task.id),
                            ),
                          ),
                          icon: const Icon(Icons.undo),
                          label: Text(
                            l10n.tasksActionReopen,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
              ),
            ],
          ),
        ),
      ],
    );
  }

  Future<void> _editText(BuildContext context, WidgetRef ref) async {
    final api = ref.read(coreApiProvider);
    final text = await showDialog<String>(
      context: context,
      builder: (_) => _EditTextDialog(initial: task.description),
    );
    if (text == null || !context.mounted) return;
    await forwardIntent(
      context,
      api.updateTask(
        taskId: task.id,
        patch: TaskPatch(text: text, clearDue: false, clearRecurrence: false),
      ),
    );
  }
}

class _EditTextDialog extends HookWidget {
  const new({required this.initial});

  final String initial;

  @override
  Widget build(BuildContext context) {
    final controller = useTextEditingController(text: initial);
    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          return AlertDialog(
            title: Text(l10n.tasksActionEditText),
            content: TextField(
              controller: controller,
              autofocus: true,
              maxLines: null,
              decoration: InputDecoration(labelText: l10n.editorTextLabel),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(context),
                child: Text(
                  l10n.commonCancel,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              FilledButton(
                style: tallFilledButton,
                onPressed: () => Navigator.pop(context, controller.text),
                child: Text(
                  l10n.commonSave,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _Field extends StatelessWidget {
  const new({
    required this.icon,
    required this.label,
    required this.value,
    this.action,
  });

  final IconData icon;
  final String label;
  final Widget value;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final trailing = action;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Icon(icon, size: 18, color: colors.text2),
          ),
          const SizedBox(width: StrataSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: text.caption.copyWith(color: colors.text2)),
                const SizedBox(height: 2),
                value,
                if (trailing != null)
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: trailing,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _HistoryRow extends StatelessWidget {
  const new({required this.occurrence});

  final TaskItem occurrence;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final done = occurrence.completionLabel;
    final due = occurrence.dueLabel;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            occurrence.state == TaskState.cancelled
                ? Icons.cancel_outlined
                : Icons.check_circle,
            size: 18,
            color: occurrence.state == TaskState.cancelled
                ? colors.text2
                : colors.success,
          ),
          const SizedBox(width: StrataSpacing.s2),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  occurrence.state == TaskState.cancelled || done == null
                      ? l10n.tasksCancelled
                      : done,
                  style: text.bodySmall.withWeight(FontWeight.w600),
                ),
                if (due != null)
                  Text(
                    l10n.tasksDueLabel(due: due),
                    style: text.caption.copyWith(color: colors.text2),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
