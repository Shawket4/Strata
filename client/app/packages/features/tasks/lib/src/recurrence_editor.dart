import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_tasks/src/labels.dart';
import 'package:strata_ui/strata_ui.dart';

/// The recurrence editor of a task.
///
/// The rule is the Tasks-plugin phrase kept verbatim in the vault (`every
/// month on the 1st`); Save sends it unchanged with `CoreApi.updateTask`
/// (`TaskPatch.recurrence`), "Stop repeating" sends
/// `TaskPatch.clearRecurrence`. The structured builder (frequency, interval,
/// day of month / nth weekday / last day, ends) and the preview of the next
/// dates need the core's rule builder and are shown as not yet available
/// (docs/CORE_GAPS.md) — no rule is composed or evaluated in Dart (L15).
class RecurrenceEditor extends HookConsumerWidget {
  /// Creates the editor for [task].
  const new({required this.task, super.key, this.line});

  /// The task whose rule is edited.
  final TaskItem task;

  /// The stored task line, when known.
  final String? line;

  /// Presents the editor: a bottom sheet on compact, a dialog otherwise.
  static Future<void> show(
    BuildContext context, {
    required TaskItem task,
    String? line,
  }) {
    final editor = RecurrenceEditor(task: task, line: line);
    if (SizeClass.of(context) == SizeClass.compact) {
      return showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        builder: (_) => editor,
      );
    }
    return showDialog<void>(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: editor,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final phrase = useTextEditingController(text: task.recurrence ?? '');
    final value = useValueListenable(phrase);
    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          final colors = context.strataColors;
          final text = context.strataText;
          final api = ref.read(coreApiProvider);
          final stored = line;

          void save() {
            if (value.text.isEmpty) return;
            unawaited(
              forwardIntent(
                context,
                api.updateTask(
                  taskId: task.id,
                  patch: TaskPatch(
                    recurrence: value.text,
                    clearDue: false,
                    clearRecurrence: false,
                  ),
                ),
              ),
            );
            Navigator.maybePop(context);
          }

          void stop() {
            unawaited(
              forwardIntent(
                context,
                api.updateTask(
                  taskId: task.id,
                  patch: const TaskPatch(
                    clearDue: false,
                    clearRecurrence: true,
                  ),
                ),
              ),
            );
            Navigator.maybePop(context);
          }

          final section = text.caption
              .withWeight(FontWeight.w700)
              .copyWith(color: colors.text2, letterSpacing: 0.4);
          return CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.enter, control: true):
                  save,
              const SingleActivator(LogicalKeyboardKey.enter, meta: true): save,
            },
            child: SingleChildScrollView(
              padding: EdgeInsets.fromLTRB(
                StrataSpacing.s5,
                StrataSpacing.s4,
                StrataSpacing.s5,
                StrataSpacing.s5 + MediaQuery.viewInsetsOf(context).bottom,
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      Icon(Icons.repeat, color: colors.accentText),
                      const SizedBox(width: StrataSpacing.s2),
                      Expanded(
                        child: Semantics(
                          header: true,
                          container: true,
                          child: Text(
                            l10n.recurrenceTitle,
                            style: text.titleSmall,
                          ),
                        ),
                      ),
                      IconButton(
                        tooltip: l10n.recurrenceClose,
                        onPressed: () => Navigator.maybePop(context),
                        icon: const Icon(Icons.close),
                      ),
                    ],
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  TextField(
                    controller: phrase,
                    textDirection: TextDirection.ltr,
                    decoration: InputDecoration(
                      labelText: l10n.recurrencePhraseLabel,
                      hintText: l10n.recurrencePhraseHint,
                      helperText: l10n.recurrencePhraseHelper,
                      helperMaxLines: 3,
                    ),
                  ),
                  const SizedBox(height: StrataSpacing.s4),
                  Text(l10n.recurrenceFrequency, style: section),
                  const SizedBox(height: StrataSpacing.s2),
                  SegmentedButton<int>(
                    segments: [
                      ButtonSegment(
                        value: 0,
                        label: Text(l10n.recurrenceDaily),
                      ),
                      ButtonSegment(
                        value: 1,
                        label: Text(l10n.recurrenceWeekly),
                      ),
                      ButtonSegment(
                        value: 2,
                        label: Text(l10n.recurrenceMonthly),
                      ),
                      ButtonSegment(
                        value: 3,
                        label: Text(l10n.recurrenceYearly),
                      ),
                    ],
                    selected: const {},
                    emptySelectionAllowed: true,
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  Wrap(
                    spacing: StrataSpacing.s4,
                    runSpacing: StrataSpacing.s1,
                    children: [
                      for (final option in [
                        l10n.recurrenceOnDayOfMonth,
                        l10n.recurrenceOnNthWeekday,
                        l10n.recurrenceOnLastDay,
                        l10n.recurrenceEndsNever,
                        l10n.recurrenceEndsOnDate,
                        l10n.recurrenceEndsAfter,
                      ])
                        Text(
                          option,
                          style: text.bodySmall.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
                  const SizedBox(height: StrataSpacing.s2),
                  StatusPill(
                    label: l10n.recurrenceBuilderUnavailable,
                    icon: Icons.hourglass_empty,
                  ),
                  const SizedBox(height: StrataSpacing.s4),
                  Text(l10n.recurrencePreviewTitle, style: section),
                  const SizedBox(height: StrataSpacing.s1),
                  Text(
                    l10n.recurrencePreviewUnavailable,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                  if (stored != null) ...[
                    const SizedBox(height: StrataSpacing.s4),
                    Text(l10n.tasksFieldStoredLine, style: section),
                    const SizedBox(height: StrataSpacing.s1),
                    Container(
                      padding: const EdgeInsets.all(StrataSpacing.s2),
                      decoration: BoxDecoration(
                        color: colors.surface2,
                        borderRadius: StrataRadii.inputRadius,
                      ),
                      child: Text(
                        stored,
                        textDirection: TextDirection.ltr,
                        style: text.monoSmall,
                      ),
                    ),
                  ],
                  const SizedBox(height: StrataSpacing.s5),
                  Wrap(
                    alignment: WrapAlignment.end,
                    spacing: StrataSpacing.s2,
                    runSpacing: StrataSpacing.s2,
                    children: [
                      if (task.recurrence != null)
                        TextButton(
                          onPressed: stop,
                          child: Text(
                            l10n.recurrenceStop,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      OutlinedButton(
                        onPressed: () => Navigator.maybePop(context),
                        child: Text(
                          l10n.commonCancel,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      FilledButton(
                        style: tallFilledButton,
                        onPressed: value.text.isEmpty ? null : save,
                        child: Text(
                          l10n.recurrenceSave,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}
