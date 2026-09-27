import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_home/src/capture_composer.dart';
import 'package:strata_home/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// Home / Capture (PLAN §11 screens 2 and 12): the capture composer, the
/// inbox count, the core's task sections (compact shows Overdue / Today /
/// Upcoming / Recurring here, D28; medium and expanded show a Today block in
/// the secondary column), recent notes, and the AI activity feed and
/// open-items roll-up (rendered as not yet available until the core streams
/// them). Ctrl/⌘+N focuses the composer.
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

  /// Opens a note by id.
  final ValueChanged<String>? onOpenNote;

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
        composer,
        const SizedBox(height: StrataSpacing.s3),
        if (view == null)
          _dataOr(value, content, const SizedBox.shrink())
        else ...[
          _InboxCountCard(count: view.inboxCount, onOpen: screen.onOpenInbox),
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
    final l10n = context.homeL10n;
    final view = value.value;
    return ListView(
      padding: const EdgeInsets.all(StrataSpacing.s6),
      children: [
        Semantics(
          header: true,
          container: true,
          child: Text(l10n.homeTitle, style: context.strataText.display),
        ),
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
                    _InboxCountCard(
                      count: view.inboxCount,
                      onOpen: screen.onOpenInbox,
                    ),
                    _TodayBlock(
                      sections: view.tasks,
                      onOpenTask: screen.onOpenTask,
                      onOpenTasks: screen.onOpenTasks,
                    ),
                    const SizedBox(height: StrataSpacing.s3),
                    const _NotYetAvailableCard(kind: _Pending.aiActivity),
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
              Semantics(
                header: true,
                container: true,
                child: Text(l10n.homeTitle, style: context.strataText.display),
              ),
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
                    _InboxCountCard(
                      count: view.inboxCount,
                      onOpen: screen.onOpenInbox,
                    ),
                  ],
                  const _NotYetAvailableCard(kind: _Pending.aiActivity),
                  const SizedBox(height: StrataSpacing.s3),
                  const _NotYetAvailableCard(kind: _Pending.openItems),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _InboxCountCard extends StatelessWidget {
  const new({required this.count, required this.onOpen});

  final int count;
  final VoidCallback? onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
      child: Card(
        margin: EdgeInsets.zero,
        color: count == 0 ? null : colors.accentTint,
        child: InkWell(
          borderRadius: StrataRadii.cardRadius,
          onTap: onOpen,
          child: Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Row(
              children: [
                Icon(Icons.move_to_inbox_outlined, color: colors.accentText),
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
            count: sections.today.length,
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

class _RecentNotes extends StatelessWidget {
  const new({
    required this.notes,
    required this.onOpenNote,
    required this.onOpenNotes,
    this.table = false,
  });

  final List<NoteListItem> notes;
  final ValueChanged<String>? onOpenNote;
  final VoidCallback? onOpenNotes;
  final bool table;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
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
        if (notes.isEmpty)
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s4),
            child: Text(
              l10n.homeNoNotes,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          ),
        if (table && notes.isNotEmpty)
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
        for (final note in notes)
          _NoteRow(note: note, table: table, onOpen: onOpenNote),
      ],
    );
  }
}

class _NoteRow extends StatelessWidget {
  const new({required this.note, required this.table, required this.onOpen});

  final NoteListItem note;
  final bool table;
  final ValueChanged<String>? onOpen;

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
    final edited = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Flexible(
          child: Text(l10n.homeEdited(date: note.updatedAt), style: meta),
        ),
        if (note.pendingSync) ...[
          const SizedBox(width: StrataSpacing.s1),
          const NotSyncedMarker(),
        ],
      ],
    );
    final title = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          note.title,
          textAlign: TextAlign.start,
          style: text.body.withWeight(FontWeight.w600),
        ),
        if (note.snippet.isNotEmpty)
          Text(
            note.snippet,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.start,
            style: text.bodySmall.copyWith(color: colors.text2),
          ),
      ],
    );
    return InkWell(
      onTap: open == null ? null : () => open(note.id),
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

enum _Pending { aiActivity, openItems }

/// A block the core does not stream yet (AI activity feed, open-items
/// roll-up): its title and the "not yet available" state.
class _NotYetAvailableCard extends StatelessWidget {
  const new({required this.kind});

  final _Pending kind;

  @override
  Widget build(BuildContext context) {
    final l10n = context.homeL10n;
    final text = context.strataText;
    final colors = context.strataColors;
    final (title, message, icon) = switch (kind) {
      _Pending.aiActivity => (
        l10n.homeAiActivity,
        l10n.homeAiActivityUnavailable,
        Icons.auto_awesome_outlined,
      ),
      _Pending.openItems => (
        l10n.homeOpenItems,
        l10n.homeOpenItemsUnavailable,
        Icons.checklist,
      ),
    };
    return _SectionCard(
      title: title,
      icon: icon,
      children: [
        Padding(
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
                label: l10n.homeNotYetAvailable,
                icon: Icons.hourglass_empty,
              ),
              const SizedBox(height: StrataSpacing.s2),
              Text(
                message,
                style: text.bodySmall.copyWith(color: colors.text2),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
