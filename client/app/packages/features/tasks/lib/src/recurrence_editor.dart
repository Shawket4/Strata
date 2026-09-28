import 'dart:async';
import 'dart:typed_data';

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
/// month on the 1st`). The builder (frequency, interval, weekdays, the day
/// rule of a month, "when done") is the core's form of the phrase
/// (`recurrence_form`); every change is compiled back by the core
/// (`compose_recurrence`) into the phrase and its summary, and the next dates
/// come from the core too (`recurrence_preview`) — no rule is composed or
/// evaluated in Dart (L15). Save sends the phrase with `CoreApi.updateTask`
/// (`TaskPatch.recurrence`), "Stop repeating" sends
/// `TaskPatch.clearRecurrence`.
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
    final api = ref.read(coreApiProvider);
    final form = useState<RecurrenceForm?>(null);
    final summary = useState<String?>(null);
    // The builder starts from the core's reading of the stored phrase.
    useEffect(() {
      final initial = task.recurrence;
      if (initial == null) return null;
      unawaited(
        api
            .recurrenceForm(phrase: initial)
            .then((parsed) {
              if (context.mounted) form.value = parsed;
            })
            .catchError((Object _) {}),
      );
      return null;
    }, const []);

    Future<void> setForm(RecurrenceForm next) async {
      form.value = next;
      try {
        final composed = await api.composeRecurrence(form: next);
        if (!context.mounted) return;
        phrase.text = composed.phrase;
        summary.value = composed.label;
      } on Object {
        // The typed phrase stays as it is.
      }
    }

    Future<void> onPhrase(String text) async {
      summary.value = null;
      try {
        final parsed = await api.recurrenceForm(phrase: text);
        if (context.mounted) form.value = parsed;
      } on Object {
        // Not understood: the builder keeps its last state.
      }
    }

    final due = task.due;
    final preview = value.text.isEmpty || due == null
        ? null
        : ref.watch(recurrencePreviewProvider(value.text, due, 3));

    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          final colors = context.strataColors;
          final text = context.strataText;
          final stored = line;
          final current = form.value;

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
                    onChanged: (text) => unawaited(onPhrase(text)),
                    decoration: InputDecoration(
                      labelText: l10n.recurrencePhraseLabel,
                      hintText: l10n.recurrencePhraseHint,
                      helperText: l10n.recurrencePhraseHelper,
                      helperMaxLines: 3,
                    ),
                  ),
                  if (summary.value case final label?)
                    Padding(
                      padding: const EdgeInsets.only(top: StrataSpacing.s1),
                      child: Text(
                        label,
                        style: text.bodySmall.withWeight(FontWeight.w600),
                      ),
                    ),
                  const SizedBox(height: StrataSpacing.s4),
                  Text(l10n.recurrenceFrequency, style: section),
                  const SizedBox(height: StrataSpacing.s2),
                  _FormBuilder(
                    form: current,
                    onChanged: (next) => unawaited(setForm(next)),
                  ),
                  const SizedBox(height: StrataSpacing.s4),
                  Text(l10n.recurrencePreviewTitle, style: section),
                  const SizedBox(height: StrataSpacing.s1),
                  switch (preview) {
                    AsyncData(:final value) when value.isNotEmpty => Wrap(
                      spacing: StrataSpacing.s2,
                      runSpacing: StrataSpacing.s1,
                      children: [
                        for (final item in value)
                          StatusPill(
                            label: item.label,
                            tone: item.isDue
                                ? StatusTone.info
                                : StatusTone.neutral,
                            icon: item.isDue ? Icons.event : null,
                          ),
                      ],
                    ),
                    _ => Text(
                      l10n.recurrencePreviewNone,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  },
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

/// The structured rule: frequency, interval, weekdays (weekly), the day rule
/// (monthly and yearly) and "when done". Each change is a new form for the
/// core to compile.
class _FormBuilder extends StatelessWidget {
  const new({required this.form, required this.onChanged});

  final RecurrenceForm? form;
  final ValueChanged<RecurrenceForm> onChanged;

  static final RecurrenceForm _default = RecurrenceForm(
    frequency: RecurrenceFrequency.weekly,
    interval: 1,
    weekdays: [],
    monthDayMode: MonthDayMode.sameDay,
    monthDays: Uint32List(0),
    nth: 1,
    months: Uint32List(0),
    whenDone: false,
  );

  RecurrenceForm _with({
    RecurrenceFrequency? frequency,
    int? interval,
    List<WeekdayKind>? weekdays,
    MonthDayMode? monthDayMode,
    Uint32List? monthDays,
    int? nth,
    WeekdayKind? nthWeekday,
    bool? whenDone,
  }) {
    final f = form ?? _default;
    return RecurrenceForm(
      frequency: frequency ?? f.frequency,
      interval: interval ?? f.interval,
      weekdays: weekdays ?? f.weekdays,
      monthDayMode: monthDayMode ?? f.monthDayMode,
      monthDays: monthDays ?? f.monthDays,
      nth: nth ?? f.nth,
      nthWeekday: nthWeekday ?? f.nthWeekday,
      months: f.months,
      whenDone: whenDone ?? f.whenDone,
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.tasksL10n;
    final text = context.strataText;
    final colors = context.strataColors;
    final f = form;
    String weekday(WeekdayKind day) => switch (day) {
      WeekdayKind.mon => l10n.weekdayMon,
      WeekdayKind.tue => l10n.weekdayTue,
      WeekdayKind.wed => l10n.weekdayWed,
      WeekdayKind.thu => l10n.weekdayThu,
      WeekdayKind.fri => l10n.weekdayFri,
      WeekdayKind.sat => l10n.weekdaySat,
      WeekdayKind.sun => l10n.weekdaySun,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SegmentedButton<RecurrenceFrequency>(
          segments: [
            ButtonSegment(
              value: RecurrenceFrequency.daily,
              label: Text(l10n.recurrenceDaily),
            ),
            ButtonSegment(
              value: RecurrenceFrequency.weekly,
              label: Text(l10n.recurrenceWeekly),
            ),
            ButtonSegment(
              value: RecurrenceFrequency.monthly,
              label: Text(l10n.recurrenceMonthly),
            ),
            ButtonSegment(
              value: RecurrenceFrequency.yearly,
              label: Text(l10n.recurrenceYearly),
            ),
          ],
          selected: {?f?.frequency},
          emptySelectionAllowed: true,
          showSelectedIcon: false,
          onSelectionChanged: (values) {
            if (values.isEmpty) return;
            onChanged(_with(frequency: values.first));
          },
        ),
        if (f != null) ...[
          const SizedBox(height: StrataSpacing.s2),
          Row(
            children: [
              Expanded(
                child: Text(
                  l10n.recurrenceInterval(count: f.interval),
                  style: text.bodySmall,
                ),
              ),
              IconButton(
                tooltip: l10n.recurrenceIntervalLess,
                onPressed: f.interval <= 1
                    ? null
                    : () => onChanged(_with(interval: f.interval - 1)),
                icon: const Icon(Icons.remove),
              ),
              IconButton(
                tooltip: l10n.recurrenceIntervalMore,
                onPressed: () => onChanged(_with(interval: f.interval + 1)),
                icon: const Icon(Icons.add),
              ),
            ],
          ),
          if (f.frequency == RecurrenceFrequency.weekly)
            Wrap(
              spacing: StrataSpacing.s1,
              runSpacing: StrataSpacing.s1,
              children: [
                for (final day in WeekdayKind.values)
                  FilterChip(
                    label: Text(weekday(day)),
                    selected: f.weekdays.contains(day),
                    onSelected: (on) => onChanged(
                      _with(
                        weekdays: [
                          for (final d in WeekdayKind.values)
                            if (d == day ? on : f.weekdays.contains(d)) d,
                        ],
                      ),
                    ),
                  ),
              ],
            ),
          if (f.frequency == RecurrenceFrequency.monthly ||
              f.frequency == RecurrenceFrequency.yearly)
            RadioGroup<MonthDayMode>(
              groupValue: f.monthDayMode,
              onChanged: (mode) {
                if (mode != null) onChanged(_with(monthDayMode: mode));
              },
              child: Column(
                children: [
                  RadioListTile<MonthDayMode>(
                    dense: true,
                    value: MonthDayMode.sameDay,
                    title: Text(l10n.recurrenceOnDueDay),
                  ),
                  RadioListTile<MonthDayMode>(
                    dense: true,
                    value: MonthDayMode.days,
                    title: Text(l10n.recurrenceOnDayOfMonth),
                    secondary: f.monthDayMode == MonthDayMode.days
                        ? DropdownButton<int>(
                            value: f.monthDays.isEmpty ? 1 : f.monthDays.first,
                            items: [
                              for (var d = 1; d <= 31; d++)
                                DropdownMenuItem(value: d, child: Text('$d')),
                            ],
                            onChanged: (d) {
                              if (d == null) return;
                              onChanged(
                                _with(monthDays: Uint32List.fromList([d])),
                              );
                            },
                          )
                        : null,
                  ),
                  RadioListTile<MonthDayMode>(
                    dense: true,
                    value: MonthDayMode.nthWeekday,
                    title: Text(l10n.recurrenceOnNthWeekday),
                  ),
                  if (f.monthDayMode == MonthDayMode.nthWeekday)
                    Padding(
                      padding: const EdgeInsetsDirectional.only(
                        start: StrataSpacing.s8,
                      ),
                      child: Wrap(
                        spacing: StrataSpacing.s3,
                        children: [
                          DropdownButton<int>(
                            value: f.nth,
                            items: [
                              for (final n in const [1, 2, 3, 4, -1])
                                DropdownMenuItem(
                                  value: n,
                                  child: Text(l10n.recurrenceNth(nth: '$n')),
                                ),
                            ],
                            onChanged: (n) {
                              if (n != null) onChanged(_with(nth: n));
                            },
                          ),
                          DropdownButton<WeekdayKind>(
                            value: f.nthWeekday ?? WeekdayKind.mon,
                            items: [
                              for (final day in WeekdayKind.values)
                                DropdownMenuItem(
                                  value: day,
                                  child: Text(weekday(day)),
                                ),
                            ],
                            onChanged: (day) {
                              if (day != null) {
                                onChanged(_with(nthWeekday: day));
                              }
                            },
                          ),
                        ],
                      ),
                    ),
                  RadioListTile<MonthDayMode>(
                    dense: true,
                    value: MonthDayMode.lastDay,
                    title: Text(l10n.recurrenceOnLastDay),
                  ),
                ],
              ),
            ),
          SwitchListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            value: f.whenDone,
            onChanged: (on) => onChanged(_with(whenDone: on)),
            title: Text(l10n.recurrenceWhenDone),
          ),
        ] else
          Padding(
            padding: const EdgeInsets.only(top: StrataSpacing.s2),
            child: Text(
              l10n.recurrenceNotUnderstoodHint,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          ),
      ],
    );
  }
}
