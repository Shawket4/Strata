import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_home/src/capture_composer.dart';
import 'package:strata_home/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// Home / Capture (PLAN §11 screens 2 and 12): the core's date and greeting,
/// the capture composer, the inbox with its newest captures, the core's task
/// sections (compact shows Overdue / Today / Upcoming / Recurring here, D28;
/// medium and expanded show a Today block in the secondary column), recent
/// notes with their filter, the AI activity feed (undo or retype a
/// decision, D13) and the open-items roll-up. Ctrl/⌘+N focuses the
/// composer.
class HomeScreen extends HookConsumerWidget {
  /// Creates Home.
  const new({
    super.key,
    this.onOpenNote,
    this.onOpenNotes,
    this.onOpenInbox,
    this.onOpenTasks,
    this.onOpenTask,
  });

  /// The icon that represents this feature.
  static const IconData icon = Icons.home_outlined;

  /// Opens a note (at a block for citations).
  final OpenNoteAt? onOpenNote;

  /// Opens the notes list ("All notes").
  final VoidCallback? onOpenNotes;

  /// Opens the Inbox.
  final VoidCallback? onOpenInbox;

  /// Opens the Tasks destination ("All tasks").
  final VoidCallback? onOpenTasks;

  /// Opens a task by id (compact: full-screen detail).
  final ValueChanged<String>? onOpenTask;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final value = ref.watch(homeProvider);
    final composerFocus = useFocusNode();
    // The AI activity feed is read from the server once per visit.
    useEffect(() {
      unawaited(
        ref.read(coreApiProvider).refreshAiActivity().catchError((Object _) {}),
      );
      return null;
    }, const []);
    final sizeClass = SizeClass.of(context);
    void focusComposer() => composerFocus.requestFocus();
    return HomeL10nScope(
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.keyN, control: true):
              focusComposer,
          const SingleActivator(LogicalKeyboardKey.keyN, meta: true):
              focusComposer,
        },
        child: Focus(
          autofocus: true,
          includeSemantics: false,
          child: Builder(
            builder: (context) {
              final view = value.value;
              final offline = view?.sync_.connectivity == Connectivity.offline;
              final composer = CaptureComposer(
                focusNode: composerFocus,
                offline: offline,
                minLines: sizeClass == SizeClass.compact ? 3 : 4,
              );
              final l10n = context.homeL10n;
              Widget content(Widget Function(HomeView view) build) =>
                  CoreAsyncBody(
                    value: value,
                    errorTitle: l10n.homeLoadError,
                    data: build,
                  );
              return switch (sizeClass) {
                SizeClass.compact => _CompactHome(
                  composer: composer,
                  value: value,
                  content: content,
                  screen: this,
                ),
                SizeClass.medium => _MediumHome(
                  composer: composer,
                  value: value,
                  content: content,
                  screen: this,
                ),
                SizeClass.expanded => _ExpandedHome(
                  composer: composer,
                  value: value,
                  content: content,
                  screen: this,
                ),
              };
            },
          ),
        ),
      ),
    );
  }
}

typedef _Content = Widget Function(Widget Function(HomeView view) build);

/// Renders [child] while the value has data, or the loading / error state in
/// its place (the composer stays usable either way).
Widget _dataOr(AsyncValue<HomeView> value, _Content content, Widget child) =>
    value.hasValue
    ? child
    : SizedBox(height: 240, child: content((_) => child));

class _CompactHome extends StatelessWidget {
  const new({
    required this.composer,
    required this.value,
    required this.content,
    required this.screen,
  });

  final Widget composer;
  final AsyncValue<HomeView> value;
  final _Content content;
  final HomeScreen screen;

  @override
  Widget build(BuildContext context) {
    final view = value.value;
    return ListView(
      padding: const EdgeInsets.all(StrataSpacing.s4),
      children: [
        if (view != null) _Header(view: view, compact: true),
        composer,
        const SizedBox(height: StrataSpacing.s3),
        if (view == null)
          _dataOr(value, content, const SizedBox.shrink())
        else ...[
          _InboxCard(view: view, onOpen: screen.onOpenInbox),
          _TaskSections(
            sections: view.tasks,
            onOpenTask: screen.onOpenTask,
            onOpenTasks: screen.onOpenTasks,
          ),
          const SizedBox(height: StrataSpacing.s2),
          _RecentNotes(
            notes: view.recentNotes,
            onOpenNote: screen.onOpenNote,
            onOpenNotes: screen.onOpenNotes,
          ),
          _AiActivityCard(view: view),
          _OpenItemsCard(view: view, onOpenNote: screen.onOpenNote),
        ],
      ],
    );
  }
}

class _MediumHome extends StatelessWidget {
  const new({
    required this.composer,
    required this.value,
    required this.content,
    required this.screen,
  });

  final Widget composer;
  final AsyncValue<HomeView> value;
  final _Content content;
  final HomeScreen screen;

  @override
  Widget build(BuildContext context) {
    final view = value.value;
    return ListView(
      padding: const EdgeInsets.all(StrataSpacing.s6),
      children: [
        _Header(view: view),
        const SizedBox(height: StrataSpacing.s4),
        composer,
        const SizedBox(height: StrataSpacing.s5),
        if (view == null)
          _dataOr(value, content, const SizedBox.shrink())
        else
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: _RecentNotes(
                  notes: view.recentNotes,
                  onOpenNote: screen.onOpenNote,
                  onOpenNotes: screen.onOpenNotes,
                ),
              ),
              const SizedBox(width: StrataSpacing.s5),
              SizedBox(
                width: 320,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _InboxCard(view: view, onOpen: screen.onOpenInbox),
                    _TodayBlock(
                      sections: view.tasks,
                      onOpenTask: screen.onOpenTask,
                      onOpenTasks: screen.onOpenTasks,
                    ),
                    const SizedBox(height: StrataSpacing.s3),
                    _AiActivityCard(view: view),
                    _OpenItemsCard(view: view, onOpenNote: screen.onOpenNote),
                  ],
                ),
              ),
            ],
          ),
      ],
    );
  }
}

class _ExpandedHome extends StatelessWidget {
  const new({
    required this.composer,
    required this.value,
    required this.content,
    required this.screen,
  });

  final Widget composer;
  final AsyncValue<HomeView> value;
  final _Content content;
  final HomeScreen screen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final view = value.value;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.all(StrataSpacing.s8),
            children: [
              _Header(view: view),
              const SizedBox(height: StrataSpacing.s4),
              composer,
              const SizedBox(height: StrataSpacing.s6),
              if (view == null)
                _dataOr(value, content, const SizedBox.shrink())
              else
                _RecentNotes(
                  notes: view.recentNotes,
                  onOpenNote: screen.onOpenNote,
                  onOpenNotes: screen.onOpenNotes,
                  table: true,
                ),
            ],
          ),
        ),
        VerticalDivider(width: 1, color: colors.border),
        SizedBox(
          width: StrataLayout.contextPanelWidth,
          child: Semantics(
            container: true,
            label: l10n.homeSecondaryLabel,
            child: ColoredBox(
              color: colors.surface,
              child: ListView(
                padding: const EdgeInsets.all(StrataSpacing.s4),
                children: [
                  if (view != null) ...[
                    _TodayBlock(
                      sections: view.tasks,
                      onOpenTask: screen.onOpenTask,
                      onOpenTasks: screen.onOpenTasks,
                    ),
                    const SizedBox(height: StrataSpacing.s3),
                    _InboxCard(view: view, onOpen: screen.onOpenInbox),
                    _AiActivityCard(view: view),
                    _OpenItemsCard(view: view, onOpenNote: screen.onOpenNote),
                  ],
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// The core's date and greeting ("Good morning, Shawket").
class _Header extends StatelessWidget {
  const new({required this.view, this.compact = false});

  final HomeView? view;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final text = context.strataText;
    final colors = context.strataColors;
    final shown = view;
    return Padding(
      padding: EdgeInsets.only(bottom: compact ? StrataSpacing.s3 : 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (shown != null && shown.todayLabel.isNotEmpty)
            Text(
              shown.todayLabel,
              style: text.caption.copyWith(color: colors.text2),
            ),
          Semantics(
            header: true,
            container: true,
            child: Text(
              shown == null || shown.greeting.isEmpty
                  ? l10n.homeTitle
                  : shown.greeting,
              style: compact ? text.title : text.display,
            ),
          ),
        ],
      ),
    );
  }
}

/// The inbox: the count, the core's summary, who needs you, and the newest
/// captures with what the AI proposes.
class _InboxCard extends StatelessWidget {
  const new({required this.view, required this.onOpen});

  final HomeView view;
  final VoidCallback? onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final count = view.inboxCount;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
      child: Card(
        margin: EdgeInsets.zero,
        clipBehavior: Clip.antiAlias,
        color: count == 0 ? null : colors.accentTint,
        child: InkWell(
          onTap: onOpen,
          child: Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Icon(
                      Icons.move_to_inbox_outlined,
                      color: colors.accentText,
                    ),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(
                      child: Text(
                        l10n.homeInboxWaiting(count: count),
                        style: text.bodySmall
                            .withWeight(FontWeight.w600)
                            .copyWith(color: colors.accentText),
                      ),
                    ),
                    Icon(
                      Directionality.of(context) == TextDirection.rtl
                          ? Icons.chevron_left
                          : Icons.chevron_right,
                      color: colors.accentText,
                    ),
                  ],
                ),
                if (view.inboxSummary.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: StrataSpacing.s1),
                    child: Text(
                      view.inboxSummary,
                      style: text.caption.copyWith(color: colors.text),
                    ),
                  ),
                if (view.needsYouCount > 0 || view.contradictionsCount > 0)
                  Padding(
                    padding: const EdgeInsets.only(top: StrataSpacing.s2),
                    child: Wrap(
                      spacing: StrataSpacing.s2,
                      runSpacing: StrataSpacing.s1,
                      children: [
                        if (view.needsYouCount > 0)
                          StatusPill(
                            label: l10n.homeNeedsYou(count: view.needsYouCount),
                            tone: StatusTone.warning,
                            icon: Icons.front_hand_outlined,
                          ),
                        if (view.contradictionsCount > 0)
                          StatusPill(
                            label: l10n.homeContradictions(
                              count: view.contradictionsCount,
                            ),
                            tone: StatusTone.danger,
                            icon: Icons.report_problem_outlined,
                          ),
                      ],
                    ),
                  ),
                for (final item in view.inboxPreview)
                  Padding(
                    padding: const EdgeInsets.only(top: StrataSpacing.s3),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Text(
                          item.text,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          textDirection: textDirectionOf(item.textDir),
                          textAlign: TextAlign.start,
                          style: text.bodySmall.copyWith(color: colors.text),
                        ),
                        Row(
                          children: [
                            Icon(
                              item.needsYou
                                  ? Icons.front_hand_outlined
                                  : Icons.auto_awesome_outlined,
                              size: 14,
                              color: colors.text2,
                            ),
                            const SizedBox(width: StrataSpacing.s1),
                            Expanded(
                              child: Text(
                                item.summary,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: text.caption.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Compact Home's task sections, each one field of the core's
/// `TaskSections` (D28).
class _TaskSections extends StatelessWidget {
  const new({
    required this.sections,
    required this.onOpenTask,
    required this.onOpenTasks,
  });

  final TaskSections sections;
  final ValueChanged<String>? onOpenTask;
  final VoidCallback? onOpenTasks;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final tasks = context.tasksL10n;
    final empty =
        sections.overdue.isEmpty &&
        sections.today.isEmpty &&
        sections.upcoming.isEmpty &&
        sections.recurring.isEmpty;
    if (empty) {
      return _SectionCard(
        title: tasks.tasksTitle,
        children: [
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Text(
              l10n.homeNoOpenTasks,
              style: context.strataText.bodySmall.copyWith(
                color: context.strataColors.text2,
              ),
            ),
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (sections.overdue.isNotEmpty)
          _SectionCard(
            title: tasks.tasksTabOverdue,
            count: sections.overdue.length,
            children: [
              for (final task in sections.overdue)
                TaskRow(task: task, overdue: true, onOpen: onOpenTask),
            ],
          ),
        if (sections.today.isNotEmpty)
          _SectionCard(
            title: tasks.tasksTabToday,
            count: sections.todayCount,
            children: [
              for (final task in sections.today)
                TaskRow(task: task, onOpen: onOpenTask),
            ],
          ),
        if (sections.upcoming.isNotEmpty)
          _SectionCard(
            title: tasks.tasksTabUpcoming,
            count: sections.upcoming.length,
            action: onOpenTasks == null
                ? null
                : TextButton(
                    onPressed: onOpenTasks,
                    child: Text(
                      l10n.homeAllTasks,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
            children: [
              for (final task in sections.upcoming)
                TaskRow(task: task, onOpen: onOpenTask),
            ],
          ),
        if (sections.recurring.isNotEmpty)
          _SectionCard(
            title: tasks.tasksTabRecurring,
            count: sections.recurring.length,
            children: [
              for (final task in sections.recurring)
                RecurringRuleRow(task: task, onOpen: onOpenTask),
            ],
          ),
      ],
    );
  }
}

/// The Today block of the secondary column: the core's today section, then
/// its overdue section.
class _TodayBlock extends StatelessWidget {
  const new({
    required this.sections,
    required this.onOpenTask,
    required this.onOpenTasks,
  });

  final TaskSections sections;
  final ValueChanged<String>? onOpenTask;
  final VoidCallback? onOpenTasks;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final tasks = context.tasksL10n;
    final empty = sections.today.isEmpty && sections.overdue.isEmpty;
    return _SectionCard(
      title: tasks.tasksTabToday,
      icon: Icons.check_circle_outline,
      action: onOpenTasks == null
          ? null
          : TextButton(
              onPressed: onOpenTasks,
              child: Text(
                l10n.homeAllTasks,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
      children: [
        if (empty)
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Text(
              l10n.homeNothingToday,
              style: context.strataText.bodySmall.copyWith(
                color: context.strataColors.text2,
              ),
            ),
          ),
        for (final task in sections.today)
          TaskRow(task: task, onOpen: onOpenTask),
        for (final task in sections.overdue)
          TaskRow(task: task, overdue: true, onOpen: onOpenTask),
      ],
    );
  }
}

class _SectionCard extends StatelessWidget {
  const new({
    required this.title,
    required this.children,
    this.count,
    this.action,
    this.icon,
  });

  final String title;
  final int? count;
  final Widget? action;
  final IconData? icon;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final glyph = icon;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
      child: Card(
        margin: EdgeInsets.zero,
        clipBehavior: Clip.antiAlias,
        semanticContainer: false,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                if (glyph != null)
                  Padding(
                    padding: const EdgeInsetsDirectional.only(
                      start: StrataSpacing.s4,
                    ),
                    child: Icon(
                      glyph,
                      size: 18,
                      color: context.strataColors.accentText,
                    ),
                  ),
                Expanded(
                  child: StrataSectionHeader(
                    title: title,
                    count: count,
                    padding: EdgeInsetsDirectional.fromSTEB(
                      glyph == null ? StrataSpacing.s4 : StrataSpacing.s2,
                      StrataSpacing.s3,
                      StrataSpacing.s2,
                      StrataSpacing.s2,
                    ),
                  ),
                ),
                ?action,
              ],
            ),
            ...children,
          ],
        ),
      ),
    );
  }
}

/// Recent notes: edited (the Home view's list), created or filed by the AI
/// (`watch_recent`).
class _RecentNotes extends HookConsumerWidget {
  const new({
    required this.notes,
    required this.onOpenNote,
    required this.onOpenNotes,
    this.table = false,
  });

  final List<NoteListItem> notes;
  final OpenNoteAt? onOpenNote;
  final VoidCallback? onOpenNotes;
  final bool table;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final filter = useState(RecentFilter.edited);
    final shown = filter.value == RecentFilter.edited
        ? notes
        : ref.watch(recentProvider(filter.value)).value?.notes;
    final header = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2, letterSpacing: 0.4);
    return _SectionCard(
      title: l10n.homeRecentNotes,
      action: onOpenNotes == null
          ? null
          : TextButton(
              onPressed: onOpenNotes,
              child: Text(
                l10n.homeAllNotes,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s4),
          child: Semantics(
            label: l10n.homeRecentFilter,
            container: true,
            explicitChildNodes: true,
            child: Wrap(
              spacing: StrataSpacing.s2,
              runSpacing: StrataSpacing.s1,
              children: [
                for (final value in RecentFilter.values)
                  ChoiceChip(
                    label: Text(switch (value) {
                      RecentFilter.edited => l10n.homeRecentEdited,
                      RecentFilter.created => l10n.homeRecentCreated,
                      RecentFilter.filedByAi => l10n.homeRecentFiledByAi,
                    }),
                    selected: filter.value == value,
                    onSelected: (_) => filter.value = value,
                  ),
              ],
            ),
          ),
        ),
        if (shown == null)
          const Padding(
            padding: EdgeInsets.all(StrataSpacing.s4),
            child: Center(child: CircularProgressIndicator(strokeWidth: 3)),
          )
        else if (shown.isEmpty)
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Text(
              l10n.homeNoNotes,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          ),
        if (table && shown != null && shown.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: StrataSpacing.s4,
              vertical: StrataSpacing.s1,
            ),
            child: Row(
              children: [
                Expanded(
                  flex: 5,
                  child: Text(l10n.homeColumnNote, style: header),
                ),
                Expanded(
                  flex: 3,
                  child: Text(l10n.homeColumnPath, style: header),
                ),
                Expanded(child: Text(l10n.homeColumnEdited, style: header)),
              ],
            ),
          ),
        for (final note in shown ?? const <NoteListItem>[])
          _NoteRow(note: note, table: table, onOpen: onOpenNote),
      ],
    );
  }
}

class _NoteRow extends StatelessWidget {
  const new({required this.note, required this.table, required this.onOpen});

  final NoteListItem note;
  final bool table;
  final OpenNoteAt? onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = onOpen;
    final meta = text.caption.copyWith(color: colors.text2);
    final path = Text(
      note.path,
      textDirection: TextDirection.ltr,
      textAlign: TextAlign.start,
      style: text.monoSmall.copyWith(color: colors.text2),
    );
    final edited = Wrap(
      spacing: StrataSpacing.s2,
      runSpacing: 2,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Text(note.updatedLabel, style: meta),
        if (note.linkCount > 0)
          Text(l10n.homeLinks(count: note.linkCount), style: meta),
        if (note.pendingSync) const NotSyncedMarker(),
      ],
    );
    final title = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          note.title,
          textDirection: textDirectionOf(note.titleDir),
          textAlign: TextAlign.start,
          style: text.body.withWeight(FontWeight.w600),
        ),
        if (note.snippet.isNotEmpty)
          Text(
            note.snippet,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            textDirection: textDirectionOf(note.snippetDir),
            textAlign: TextAlign.start,
            style: text.bodySmall.copyWith(color: colors.text2),
          ),
      ],
    );
    return InkWell(
      onTap: open == null ? null : () => open(note.id, null),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s4,
          vertical: StrataSpacing.s2 + 2,
        ),
        child: table
            ? Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(flex: 5, child: title),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(flex: 3, child: path),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(child: edited),
                ],
              )
            : Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  title,
                  const SizedBox(height: 2),
                  Wrap(
                    spacing: StrataSpacing.s3,
                    runSpacing: 2,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [path, edited],
                  ),
                ],
              ),
      ),
    );
  }
}

/// Runs a Home intent and reports a failure in a snack bar.
Future<void> _run(BuildContext context, Future<void> Function() intent) async {
  final l10n = context.homeL10n;
  final messenger = ScaffoldMessenger.maybeOf(context);
  try {
    await intent();
  } on Object catch (error) {
    messenger
      ?..hideCurrentSnackBar()
      ..showSnackBar(
        SnackBar(
          content: Text(
            l10n.homeActionFailed(
              code: error is CoreFailure ? error.code : 'internal',
            ),
          ),
        ),
      );
  }
}

/// A block the core cannot fill now: offline, not yet available or not
/// allowed.
class _Unavailable extends StatelessWidget {
  const new({required this.availability, required this.message});

  final Availability availability;
  final String message;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final text = context.strataText;
    final colors = context.strataColors;
    return Padding(
      padding: const EdgeInsetsDirectional.fromSTEB(
        StrataSpacing.s4,
        0,
        StrataSpacing.s4,
        StrataSpacing.s4,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          StatusPill(
            label: switch (availability) {
              Availability.offline => l10n.homeOffline,
              Availability.notAllowed => l10n.homeNotAllowed,
              _ => l10n.homeNotYetAvailable,
            },
            icon: availability == Availability.offline
                ? Icons.cloud_off_outlined
                : Icons.hourglass_empty,
          ),
          const SizedBox(height: StrataSpacing.s2),
          Text(message, style: text.bodySmall.copyWith(color: colors.text2)),
        ],
      ),
    );
  }
}

/// The AI activity feed (the server's AI decisions): what the AI added or
/// found, with Undo (`reject_ai_decision`), for relations a new type
/// (`retype_ai_decision`) and, where the core allows it, a new target picked
/// from the core's choices (`repoint_choices`, then `repoint_ai_decision`),
/// D13.
class _AiActivityCard extends ConsumerWidget {
  const new({required this.view});

  final HomeView view;

  Future<void> _retype(
    BuildContext context,
    WidgetRef ref,
    AiActivityItem item,
  ) async {
    final l10n = context.homeL10n;
    final core = ref.read(coreApiProvider);
    final types = await core.relationTypes();
    if (!context.mounted) return;
    final chosen = await showDialog<String>(
      context: context,
      builder: (dialog) => SimpleDialog(
        title: Text(l10n.homeRetypeTitle),
        children: [
          for (final type in types)
            SimpleDialogOption(
              onPressed: () => Navigator.of(dialog).pop(type.key),
              child: Row(
                children: [
                  Expanded(child: Text(type.label)),
                  if (type.key == item.relType)
                    const Icon(Icons.check, size: 18),
                ],
              ),
            ),
        ],
      ),
    );
    if (chosen == null || chosen == item.relType || !context.mounted) return;
    await _run(
      context,
      () => core.retypeAiDecision(decisionId: item.decisionId, relType: chosen),
    );
  }

  Future<void> _repoint(
    BuildContext context,
    WidgetRef ref,
    AiActivityItem item,
  ) async {
    final core = ref.read(coreApiProvider);
    final chosen = await showDialog<String>(
      context: context,
      builder: (_) => RepointPickerDialog(item: item),
    );
    if (chosen == null || !context.mounted) return;
    await _run(
      context,
      () =>
          core.repointAiDecision(decisionId: item.decisionId, targetId: chosen),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final core = ref.read(coreApiProvider);
    return _SectionCard(
      title: l10n.homeAiActivity,
      icon: Icons.auto_awesome_outlined,
      children: [
        if (view.aiActivity != Availability.available)
          _Unavailable(
            availability: view.aiActivity,
            message: l10n.homeAiActivityUnavailable,
          )
        else ...[
          if (view.aiActivityHeadline.isNotEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s4),
              child: Text(
                view.aiActivityHeadline,
                style: text.caption.copyWith(color: colors.text2),
              ),
            ),
          if (view.aiActivityItems.isEmpty)
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Text(
                l10n.homeAiActivityEmpty,
                style: text.bodySmall.copyWith(color: colors.text2),
              ),
            ),
          for (final item in view.aiActivityItems)
            Padding(
              padding: const EdgeInsets.fromLTRB(
                StrataSpacing.s4,
                StrataSpacing.s2,
                StrataSpacing.s4,
                0,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Wrap(
                    spacing: StrataSpacing.s2,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        item.atLabel,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                      if (item.confidence case final confidence?)
                        Text(
                          l10n.homeAiConfidence(
                            value: confidence.toStringAsFixed(2),
                          ),
                          style: text.caption.copyWith(color: colors.infoText),
                        ),
                    ],
                  ),
                  Text(
                    item.summary,
                    style: text.bodySmall.copyWith(
                      color: item.reverted ? colors.text2 : colors.text,
                      decoration: item.reverted
                          ? TextDecoration.lineThrough
                          : null,
                    ),
                  ),
                  if (item.reverted)
                    Text(
                      l10n.homeAiUndone,
                      style: text.caption.copyWith(color: colors.text2),
                    )
                  else
                    Wrap(
                      alignment: WrapAlignment.end,
                      spacing: StrataSpacing.s1,
                      children: [
                        if (item.canRepoint)
                          TextButton(
                            onPressed: () =>
                                unawaited(_repoint(context, ref, item)),
                            child: Text(l10n.homeAiRepoint),
                          ),
                        if (item.relType != null)
                          TextButton(
                            onPressed: () =>
                                unawaited(_retype(context, ref, item)),
                            child: Text(l10n.homeAiRetype),
                          ),
                        TextButton(
                          onPressed: () => unawaited(
                            _run(
                              context,
                              () => core.rejectAiDecision(
                                decisionId: item.decisionId,
                              ),
                            ),
                          ),
                          child: Text(l10n.homeAiUndo),
                        ),
                      ],
                    ),
                ],
              ),
            ),
          const SizedBox(height: StrataSpacing.s2),
        ],
      ],
    );
  }
}

/// Picks a new target for an AI decision: the core's choices
/// (`repoint_choices`: notes of the current target's kind, matched by the
/// core as the user types). Pops the chosen note's ID.
class RepointPickerDialog extends HookConsumerWidget {
  /// Creates the picker for [item].
  const new({required this.item, super.key});

  /// The decision being corrected.
  final AiActivityItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final query = useState('');
    final choices = ref
        .watch(repointChoicesProvider(item.decisionId, query.value))
        .value;
    return AlertDialog(
      title: Text(
        l10n.homeRepointTitle(title: item.target?.title ?? item.summary),
      ),
      content: SizedBox(
        width: 420,
        height: 360,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              autofocus: true,
              onChanged: (value) => query.value = value,
              decoration: InputDecoration(
                isDense: true,
                hintText: l10n.homeRepointSearch,
                prefixIcon: const Icon(Icons.search, size: 20),
              ),
            ),
            const SizedBox(height: StrataSpacing.s2),
            Expanded(
              child: choices == null
                  ? const Center(child: CircularProgressIndicator())
                  : choices.isEmpty
                  ? Center(
                      child: Text(
                        l10n.homeRepointNone(query: query.value),
                        textAlign: TextAlign.center,
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                    )
                  : ListView(
                      children: [
                        for (final choice in choices)
                          ListTile(
                            dense: true,
                            title: Text(
                              choice.title,
                              textDirection: textDirectionOf(choice.titleDir),
                              textAlign: TextAlign.start,
                            ),
                            subtitle: choice.folder.isEmpty
                                ? null
                                : Text(
                                    choice.folder,
                                    textDirection: TextDirection.ltr,
                                    style: text.caption.copyWith(
                                      color: colors.text2,
                                    ),
                                  ),
                            onTap: () => Navigator.of(context).pop(choice.id),
                          ),
                      ],
                    ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.homeCancel),
        ),
      ],
    );
  }
}

/// Open items rolled up from people and company pages (`## Open items`),
/// each with its person and source. The check stays read-only: open items
/// are AI-maintained bullets without a done marker (CORE_GAPS).
class _OpenItemsCard extends StatelessWidget {
  const new({required this.view, required this.onOpenNote});

  final HomeView view;
  final OpenNoteAt? onOpenNote;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final open = onOpenNote;
    return _SectionCard(
      title: l10n.homeOpenItems,
      icon: Icons.checklist,
      count: view.openItemList.isEmpty ? null : view.openItemList.length,
      children: [
        if (view.openItems != Availability.available)
          _Unavailable(
            availability: view.openItems,
            message: l10n.homeOpenItemsUnavailable,
          )
        else if (view.openItemList.isEmpty)
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Text(
              l10n.homeOpenItemsEmpty,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          )
        else
          for (final item in view.openItemList)
            Padding(
              padding: const EdgeInsets.fromLTRB(
                StrataSpacing.s2,
                0,
                StrataSpacing.s4,
                StrataSpacing.s2,
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  ExcludeSemantics(
                    child: Checkbox(value: item.done, onChanged: null),
                  ),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          item.text,
                          textDirection: textDirectionOf(item.textDir),
                          textAlign: TextAlign.start,
                          style: text.bodySmall,
                        ),
                        Wrap(
                          spacing: StrataSpacing.s2,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            if (item.person.id case final personId?)
                              TextButton(
                                onPressed: open == null
                                    ? null
                                    : () => open(personId, null),
                                child: Text(item.person.title),
                              )
                            else
                              Text(
                                item.person.title,
                                style: text.caption.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            if (item.citation case final citation?)
                              CitationChip(
                                label: citation.target,
                                blockRef: citation.anchor,
                                onPressed:
                                    citation.noteId == null || open == null
                                    ? null
                                    : () => open(
                                        citation.noteId!,
                                        citation.anchor,
                                      ),
                              ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
      ],
    );
  }
}
