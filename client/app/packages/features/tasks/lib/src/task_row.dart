import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_ui/strata_ui.dart';

/// The done checkbox of a task: completes an open task
/// (`CoreApi.completeTask`; a recurring one gets its next occurrence from the
/// core) and reopens a done or cancelled one (`CoreApi.reopenTask`).
class TaskCheckbox extends ConsumerWidget {
  /// Creates the checkbox for [task].
  const new({required this.task, super.key});

  /// The task.
  final TaskItem task;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.tasksL10n;
    final open = task.state == TaskState.open;
    return Checkbox(
      value: !open,
      semanticLabel: open
          ? l10n.tasksMarkDoneSemantics(title: task.description)
          : l10n.tasksReopenSemantics(title: task.description),
      shape: const CircleBorder(),
      onChanged: (_) {
        final api = ref.read(coreApiProvider);
        unawaited(
          forwardIntent(
            context,
            open
                ? api.completeTask(taskId: task.id)
                : api.reopenTask(taskId: task.id),
          ),
        );
      },
    );
  }
}

/// The meta line of a task: due (or overdue) date, done date, cancelled,
/// recurrence phrase (or "not understood"), linked entities and the unsynced
/// marker — each rendered only when the view-model carries it.
class TaskMeta extends StatelessWidget {
  /// Creates the meta line.
  const new({
    required this.task,
    super.key,
    this.overdue = false,
    this.showLinks = true,
    this.showRecurrence = true,
  });

  /// The task.
  final TaskItem task;

  /// The task is in the core's overdue section.
  final bool overdue;

  /// Whether linked entities are listed.
  final bool showLinks;

  /// Whether the recurrence phrase is shown.
  final bool showRecurrence;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final style = context.strataText.caption.copyWith(color: colors.text2);
    final due = task.due;
    final done = task.done;
    final recurrence = task.recurrence;
    return Wrap(
      spacing: StrataSpacing.s2,
      runSpacing: 2,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        if (due != null)
          Text(
            overdue
                ? l10n.tasksOverdueSince(date: due)
                : l10n.tasksDueOn(date: due),
            style: overdue
                ? style.withWeight(FontWeight.w600).copyWith(
                    color: colors.dangerText,
                  )
                : style,
          ),
        if (done != null) Text(l10n.tasksDoneOn(date: done), style: style),
        if (task.state == TaskState.cancelled)
          Text(l10n.tasksCancelled, style: style),
        if (showRecurrence && recurrence != null)
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.repeat, size: 14, color: colors.text2),
              const SizedBox(width: 2),
              Flexible(child: Text(recurrence, style: style)),
            ],
          ),
        if (!task.recurrenceUnderstood)
          StatusPill(
            label: l10n.tasksRecurrenceNotUnderstood,
            tone: StatusTone.warning,
          ),
        if (showLinks)
          for (final link in task.links)
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.link, size: 14, color: colors.text2),
                const SizedBox(width: 2),
                Flexible(child: Text(link.title, style: style)),
              ],
            ),
        if (task.pendingSync) const NotSyncedMarker(),
      ],
    );
  }
}

/// A small "not synced yet" glyph with its semantics label.
class NotSyncedMarker extends StatelessWidget {
  /// Creates the marker.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    final label = context.tasksL10n.commonNotSynced;
    return Tooltip(
      message: label,
      excludeFromSemantics: true,
      child: Semantics(
        label: label,
        child: Icon(
          Icons.cloud_upload_outlined,
          size: 14,
          color: context.strataColors.text2,
        ),
      ),
    );
  }
}

/// The reminder bell with each reminder's wall-clock time as written.
class TaskReminderBell extends StatelessWidget {
  /// Creates the bell for [reminders].
  const new({required this.reminders, super.key});

  /// The task's reminders.
  final List<ReminderItem> reminders;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final style = context.strataText.monoSmall.copyWith(color: colors.text2);
    return Wrap(
      spacing: StrataSpacing.s1,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        for (final reminder in reminders)
          Semantics(
            label: l10n.tasksReminderSemantics(time: reminder.local),
            excludeSemantics: true,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  Icons.notifications_none_rounded,
                  size: 16,
                  color: colors.text2,
                ),
                const SizedBox(width: 2),
                Text(
                  reminder.local,
                  style: style,
                  textDirection: TextDirection.ltr,
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// One task in a list: checkbox, description, meta line and reminder bell.
/// Activating the row opens the task.
class TaskRow extends StatelessWidget {
  /// Creates the row.
  const new({
    required this.task,
    super.key,
    this.onOpen,
    this.overdue = false,
    this.selected = false,
  });

  /// The task.
  final TaskItem task;

  /// Opens the task (detail pane or screen).
  final ValueChanged<String>? onOpen;

  /// The task is in the core's overdue section.
  final bool overdue;

  /// Highlighted as the keyboard / detail selection.
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final closed = task.state != TaskState.open;
    final open = onOpen;
    return Material(
      color: selected ? colors.accentTint : Colors.transparent,
      child: InkWell(
        onTap: open == null ? null : () => open(task.id),
        child: Padding(
          padding: const EdgeInsetsDirectional.fromSTEB(
            StrataSpacing.s1,
            StrataSpacing.s1,
            StrataSpacing.s3,
            StrataSpacing.s1,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TaskCheckbox(task: task),
              const SizedBox(width: StrataSpacing.s1),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    vertical: StrataSpacing.s2 + 2,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        task.description,
                        textAlign: TextAlign.start,
                        style: text.body
                            .withWeight(FontWeight.w500)
                            .copyWith(
                              color: closed ? colors.text2 : colors.text,
                              decoration: closed
                                  ? TextDecoration.lineThrough
                                  : null,
                            ),
                      ),
                      const SizedBox(height: 2),
                      TaskMeta(task: task, overdue: overdue),
                    ],
                  ),
                ),
              ),
              if (task.reminders.isNotEmpty)
                Padding(
                  padding: const EdgeInsetsDirectional.only(
                    start: StrataSpacing.s2,
                    top: StrataSpacing.s3,
                  ),
                  child: TaskReminderBell(reminders: task.reminders),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A recurring rule summary: the task, its phrase as written and its next
/// occurrence (the open occurrence's due date).
class RecurringRuleRow extends StatelessWidget {
  /// Creates the row.
  const new({required this.task, super.key, this.onOpen});

  /// The open occurrence of the recurring task.
  final TaskItem task;

  /// Opens the task.
  final ValueChanged<String>? onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final due = task.due;
    final open = onOpen;
    return InkWell(
      onTap: open == null ? null : () => open(task.id),
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: StrataLayout.minTouchTarget),
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: StrataSpacing.s4,
            vertical: StrataSpacing.s2,
          ),
          child: Row(
            children: [
              Container(
                width: 32,
                height: 32,
                decoration: BoxDecoration(
                  color: colors.accentTint,
                  borderRadius: StrataRadii.inputRadius,
                ),
                child: Icon(Icons.repeat, size: 18, color: colors.accentText),
              ),
              const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      task.description,
                      style: text.bodySmall.withWeight(FontWeight.w600),
                    ),
                    Text(
                      due == null
                          ? task.recurrence ?? ''
                          : l10n.tasksRuleNext(
                              rule: task.recurrence ?? '',
                              date: due,
                            ),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ],
                ),
              ),
              Icon(
                Directionality.of(context) == TextDirection.rtl
                    ? Icons.chevron_left
                    : Icons.chevron_right,
                color: colors.text2,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
