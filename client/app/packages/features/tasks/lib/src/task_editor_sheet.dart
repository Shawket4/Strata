import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/duplicate_prompt.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_ui/strata_ui.dart';

/// The new-task sheet: the task text as typed, the details the user sets
/// (due date, repeat phrase, reminders) and the home note, saved with
/// `CoreApi.createTask` (duplicate-checked; the "Already exists" content is
/// shown in place when the core returns candidates, and Create anyway resends
/// with `force: true`).
///
/// "Understood as" chips need the core's parser of natural task text; until
/// it exists the section says so (docs/CORE_GAPS.md) — nothing is parsed in
/// Dart (L15).
class TaskEditorSheet extends HookConsumerWidget {
  /// Creates the sheet.
  const new({
    super.key,
    this.noteId,
    this.noteTitle,
    this.onOpenExisting,
    this.onCreated,
  });

  /// The note to add the task to (`null`: the default task list).
  final String? noteId;

  /// That note's title, for display.
  final String? noteTitle;

  /// Opens an existing candidate from the duplicate check.
  final ValueChanged<CandidateItem>? onOpenExisting;

  /// Called with the new task's id once the core created it.
  final ValueChanged<String>? onCreated;

  /// Presents the sheet: full-height bottom sheet on compact, a dialog
  /// otherwise.
  static Future<void> show(
    BuildContext context, {
    String? noteId,
    String? noteTitle,
    ValueChanged<CandidateItem>? onOpenExisting,
    ValueChanged<String>? onCreated,
  }) {
    final sheet = TaskEditorSheet(
      noteId: noteId,
      noteTitle: noteTitle,
      onOpenExisting: onOpenExisting,
      onCreated: onCreated,
    );
    if (SizeClass.of(context) == SizeClass.compact) {
      return showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        builder: (_) => sheet,
      );
    }
    return showDialog<void>(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560, maxHeight: 720),
          child: sheet,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final description = useTextEditingController();
    final recurrence = useTextEditingController();
    final text = useValueListenable(description);
    final due = useState<DateTime?>(null);
    final reminders = useState<List<DateTime>>(const []);
    final candidates = useState<List<CandidateItem>?>(null);

    TaskDraft draft() => TaskDraft(
      noteId: noteId,
      description: description.text,
      due: due.value,
      recurrence: recurrence.text.isEmpty ? null : recurrence.text,
      reminders: reminders.value,
    );

    Future<void> create({required bool force}) async {
      final api = ref.read(coreApiProvider);
      final messenger = ScaffoldMessenger.maybeOf(context);
      final l10n = lookupTasksLocalizations(Localizations.localeOf(context));
      try {
        final outcome = await api.createTask(draft: draft(), force: force);
        final id = outcome.id;
        if (id != null) {
          onCreated?.call(id);
          if (context.mounted) await Navigator.maybePop(context);
          return;
        }
        candidates.value = outcome.candidates;
      } on Object catch (error) {
        messenger?.showSnackBar(
          SnackBar(
            content: Text(
              error is CoreFailure
                  ? l10n.commonIntentFailedCode(code: error.code)
                  : l10n.editorSaveFailed,
            ),
          ),
        );
      }
    }

    void save() {
      if (text.text.isEmpty) return;
      unawaited(create(force: false));
    }

    Future<void> pickDue() async {
      final picked = await showDatePicker(
        context: context,
        initialDate: due.value,
        firstDate: DateTime(2000),
        lastDate: DateTime(2100),
      );
      if (picked == null) return;
      due.value = DateTime.utc(picked.year, picked.month, picked.day);
    }

    Future<void> addReminder() async {
      final day = await showDatePicker(
        context: context,
        initialDate: due.value,
        firstDate: DateTime(2000),
        lastDate: DateTime(2100),
      );
      if (day == null || !context.mounted) return;
      final time = await showTimePicker(
        context: context,
        initialTime: const TimeOfDay(hour: 9, minute: 0),
      );
      if (time == null) return;
      reminders.value = [
        ...reminders.value,
        DateTime.utc(day.year, day.month, day.day, time.hour, time.minute),
      ];
    }

    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          final found = candidates.value;
          if (found != null) {
            return SingleChildScrollView(
              child: DuplicateCandidatesView(
                kind: 'task',
                title: description.text,
                candidates: found,
                onOpenExisting: (candidate) {
                  onOpenExisting?.call(candidate);
                  Navigator.maybePop(context);
                },
                onCreateAnyway: () => unawaited(create(force: true)),
                onCancel: () => candidates.value = null,
              ),
            );
          }
          final colors = context.strataColors;
          final style = context.strataText;
          final dueValue = due.value;
          final section = style.caption
              .withWeight(FontWeight.w700)
              .copyWith(color: colors.text2, letterSpacing: 0.4);
          return CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.enter, control: true):
                  save,
              const SingleActivator(LogicalKeyboardKey.enter, meta: true): save,
            },
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: StrataSpacing.s1,
                    vertical: StrataSpacing.s1,
                  ),
                  child: Row(
                    children: [
                      IconButton(
                        tooltip: l10n.editorClose,
                        onPressed: () => Navigator.maybePop(context),
                        icon: const Icon(Icons.close),
                      ),
                      Expanded(
                        child: Semantics(
                          header: true,
                          container: true,
                          child: Text(
                            l10n.editorNewTaskTitle,
                            style: style.titleSmall,
                          ),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsetsDirectional.only(
                          end: StrataSpacing.s2,
                        ),
                        child: FilledButton(
                          style: tallFilledButton,
                          onPressed: text.text.isEmpty ? null : save,
                          child: Text(
                            l10n.commonSave,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                Divider(height: 1, color: colors.border),
                Flexible(
                  child: SingleChildScrollView(
                    padding: EdgeInsets.fromLTRB(
                      StrataSpacing.s4,
                      StrataSpacing.s4,
                      StrataSpacing.s4,
                      StrataSpacing.s4 +
                          MediaQuery.viewInsetsOf(context).bottom,
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        TextField(
                          controller: description,
                          autofocus: true,
                          minLines: 2,
                          maxLines: 6,
                          decoration: InputDecoration(
                            labelText: l10n.editorTextLabel,
                            hintText: l10n.editorTextHint,
                          ),
                        ),
                        const SizedBox(height: StrataSpacing.s4),
                        Text(l10n.editorParsedTitle, style: section),
                        const SizedBox(height: StrataSpacing.s1),
                        StatusPill(
                          label: l10n.editorParsedUnavailable,
                          icon: Icons.auto_awesome_outlined,
                        ),
                        const SizedBox(height: StrataSpacing.s4),
                        _EditorRow(
                          icon: Icons.event,
                          label: l10n.editorDue,
                          value: dueValue == null
                              ? l10n.tasksNoDueDate
                              : l10n.tasksDateLong(date: dueValue),
                          actions: [
                            TextButton(
                              onPressed: pickDue,
                              child: Text(
                                l10n.editorPickDate,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            if (dueValue != null)
                              IconButton(
                                tooltip: l10n.tasksActionClearDue,
                                onPressed: () => due.value = null,
                                icon: const Icon(Icons.event_busy_outlined),
                              ),
                          ],
                        ),
                        const SizedBox(height: StrataSpacing.s2),
                        TextField(
                          controller: recurrence,
                          textDirection: TextDirection.ltr,
                          decoration: InputDecoration(
                            prefixIcon: const Icon(Icons.repeat),
                            labelText: l10n.editorRepeat,
                            hintText: l10n.recurrencePhraseHint,
                          ),
                        ),
                        const SizedBox(height: StrataSpacing.s3),
                        _EditorRow(
                          icon: Icons.notifications_none_rounded,
                          label: l10n.editorReminders,
                          value: reminders.value.isEmpty
                              ? l10n.tasksNoReminders
                              : null,
                          actions: [
                            TextButton.icon(
                              onPressed: addReminder,
                              icon: const Icon(Icons.add),
                              label: Text(
                                l10n.editorAddReminder,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ],
                        ),
                        Wrap(
                          spacing: StrataSpacing.s2,
                          runSpacing: StrataSpacing.s1,
                          children: [
                            for (final at in reminders.value)
                              InputChip(
                                label: Text(l10n.editorReminderAt(at: at)),
                                deleteButtonTooltipMessage: l10n
                                    .editorRemoveReminder(
                                      time: l10n.editorReminderAt(at: at),
                                    ),
                                onDeleted: () => reminders.value = [
                                  for (final r in reminders.value)
                                    if (!identical(r, at)) r,
                                ],
                              ),
                          ],
                        ),
                        const SizedBox(height: StrataSpacing.s3),
                        _EditorRow(
                          icon: Icons.description_outlined,
                          label: l10n.editorHomeNote,
                          value: noteTitle ?? l10n.editorDefaultHome,
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _EditorRow extends StatelessWidget {
  const new({
    required this.icon,
    required this.label,
    this.value,
    this.actions = const [],
  });

  final IconData icon;
  final String label;
  final String? value;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final shown = value;
    return Row(
      children: [
        Icon(icon, size: 20, color: colors.text2),
        const SizedBox(width: StrataSpacing.s3),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(label, style: text.caption.copyWith(color: colors.text2)),
              if (shown != null)
                Text(shown, style: text.bodySmall.withWeight(FontWeight.w500)),
            ],
          ),
        ),
        ...actions,
      ],
    );
  }
}
