// Test fixtures of the editor package: notes whose editor hints are the
// spans the core's `format::hints` would emit for their content (UTF-16
// offsets), built with a small span finder so every offset is exact.
import 'package:strata_state/strata_state.dart';

/// The hint of [kind] covering the [occurrence]-th match of [text] in
/// [content].
EditorHint hintOf(
  String content,
  HintKind kind,
  String text, {
  int occurrence = 0,
}) {
  var start = -1;
  for (var i = 0; i <= occurrence; i++) {
    start = content.indexOf(text, start + 1);
    if (start < 0) throw StateError('"$text" not found in fixture');
  }
  return EditorHint(
    kind: kind,
    start: start,
    end: start + text.length,
    level: 0,
  );
}

/// The frontmatter hint of [content] as the core emits it: from the opening
/// `---` line up to and including the closing `---` line (and its
/// terminator), or none.
List<EditorHint> frontmatterHint(String content) {
  final firstEol = content.indexOf('\n');
  if (firstEol < 0) return const [];
  final first = content.substring(0, firstEol).replaceAll('\r', '');
  if (first != '---') return const [];
  var start = firstEol + 1;
  while (start <= content.length) {
    final eol = content.indexOf('\n', start);
    final end = eol < 0 ? content.length : eol;
    if (content.substring(start, end).replaceAll('\r', '') == '---') {
      return [
        EditorHint(
          kind: HintKind.frontmatter,
          start: 0,
          end: eol < 0 ? content.length : eol + 1,
          level: 0,
        ),
      ];
    }
    if (eol < 0) return const [];
    start = eol + 1;
  }
  return const [];
}

/// [hints] after [delta] UTF-16 units were inserted at [offset] (what the
/// core returns for the edited content).
List<EditorHint> shiftedHints(List<EditorHint> hints, int offset, int delta) =>
    [
      for (final h in hints)
        EditorHint(
          kind: h.kind,
          start: h.start >= offset ? h.start + delta : h.start,
          end: h.end > offset ? h.end + delta : h.end,
          level: 0,
        ),
    ];

/// Fixed IDs and texts of the sample note.
abstract final class EditorFixtures {
  /// The version the sample note was loaded at (the base of saves).
  static const String contentVersion = 'v1-5f1c0e2a';

  /// Note ID.
  static const String noteId = 'n-pricing-experiments';

  /// The sample note's markdown (SCREEN_SPEC sample content).
  static const String content =
      '---\n'
      'id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n'
      'tags: [pricing, q4]\n'
      'related: ["[[Churn notes]]"]\n'
      '---\n'
      'Three experiments to run in Q4 before we lock the '
      '[[Subscription tiers]]. [[Ahmed Samir]] runs the Acme pilot.\n'
      '\n'
      '## Hypotheses\n'
      '- Annual prepay with 2 months free lifts conversion for teams of 5+.\n'
      '- Launch offer: a **flat 10% discount** on the first annual plan. '
      '^a1b2\n'
      '- Loyalty tier for customers > 12 months. See [[Churn notes]].\n'
      '\n'
      '## Next steps\n'
      '- [ ] Draft two pricing page variants #pricing ^t-01j9p1\n'
      '- [x] Pull churn by tenure from billing ✅ 2026-09-26 ^t-01j9p2\n';

  /// The core's hints of [content].
  static final List<EditorHint> hints = [
    ...frontmatterHint(content),
    hintOf(content, HintKind.wikiLink, '[[Subscription tiers]]'),
    hintOf(content, HintKind.wikiLink, '[[Ahmed Samir]]'),
    hintOf(content, HintKind.heading, '## Hypotheses'),
    hintOf(content, HintKind.blockId, '^a1b2'),
    hintOf(content, HintKind.wikiLink, '[[Churn notes]]', occurrence: 1),
    hintOf(content, HintKind.heading, '## Next steps'),
    hintOf(
      content,
      HintKind.taskLine,
      '- [ ] Draft two pricing page variants #pricing ^t-01j9p1',
    ),
    hintOf(content, HintKind.tag, '#pricing'),
    hintOf(content, HintKind.blockId, '^t-01j9p1'),
    hintOf(
      content,
      HintKind.taskLine,
      '- [x] Pull churn by tenure from billing ✅ 2026-09-26 ^t-01j9p2',
    ),
    hintOf(content, HintKind.blockId, '^t-01j9p2'),
  ];

  /// The open task.
  static const TaskItem openTask = TaskItem(
    id: 't-01j9p1',
    noteId: noteId,
    noteTitle: 'Pricing experiments',
    description: 'Draft two pricing page variants',
    state: TaskState.open,
    priority: 'normal',
    recurrenceUnderstood: true,
    reminders: [],
    links: [],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// The done task.
  static final TaskItem doneTask = TaskItem(
    id: 't-01j9p2',
    noteId: noteId,
    noteTitle: 'Pricing experiments',
    description: 'Pull churn by tenure from billing',
    state: TaskState.done,
    priority: 'normal',
    done: DateTime.utc(2026, 9, 26),
    recurrenceUnderstood: true,
    reminders: const [],
    links: const [],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// Synced.
  static const NoteSyncState synced = NoteSyncState(
    kind: NoteSyncKind.synced,
    pendingOps: 0,
    label: '',
  );

  /// A conflict on op [conflictOpId].
  static const NoteSyncState conflict = NoteSyncState(
    kind: NoteSyncKind.conflict,
    pendingOps: 1,
    conflictOpId: conflictOpId,
    label: '',
  );

  /// The conflicting op.
  static const String conflictOpId = '01J8ZQ9CONFL1CT0000000000A';

  /// The sample note view.
  static final NoteView note = noteWith();

  /// A note view of [content] with [hints] and [sync].
  static NoteView noteWith({
    String content = EditorFixtures.content,
    List<EditorHint>? hints,
    NoteSyncState sync = synced,
    List<TaskItem>? tasks,
  }) => NoteView(
    id: noteId,
    path: 'notes/sales/Pricing experiments.md',
    title: 'Pricing experiments',
    kind: 'note',
    content: content,
    version: 'sha256:5f2c9e',
    properties: const [],
    relations: const [],
    backlinks: const [],
    tags: const ['pricing', 'q4'],
    tasks: tasks ?? [openTask, doneTask],
    hints: hints ?? EditorFixtures.hints,
    sync_: sync,
    history: Availability.available,
    titleDir: TextDir.ltr,
    contentVersion: EditorFixtures.contentVersion,
    wordCount: 0,
    backlinkCount: 0,
    historyEntries: [],
    pinned: false,
  );

  /// The note screen of [note].
  static NoteScreen screenOf(NoteView note) =>
      NoteScreen(id: noteId, note: note);

  /// Ahmed Samir in the directory.
  static const DirectoryItem ahmed = DirectoryItem(
    id: '01J8ZK0AHMED00000000000000',
    title: 'Ahmed Samir',
    subtitle: 'Operations manager, Acme Logistics',
    aliases: ['أحمد سمير'],
    kind: '',
    titleDir: TextDir.ltr,
    initials: '',
    mentionCount: 0,
    tags: [],
    location: [],
    expiringSoon: false,
    breadcrumb: [],
    documentCount: 0,
    hasOpenItems: false,
  );

  /// Acme Logistics in the directory.
  static const DirectoryItem acme = DirectoryItem(
    id: '01J8ZK0ACME000000000000000',
    title: 'Acme Logistics',
    subtitle: 'Client · logistics',
    aliases: ['أكمي'],
    kind: '',
    titleDir: TextDir.ltr,
    initials: '',
    mentionCount: 0,
    tags: [],
    location: [],
    expiringSoon: false,
    breadcrumb: [],
    documentCount: 0,
    hasOpenItems: false,
  );

  /// The people tab filtered by [query].
  static DirectoryView people(String query, List<DirectoryItem> items) =>
      DirectoryView(
        tab: DirectoryTab.people,
        query: query,
        items: items,
        counts: const DirectoryCounts(
          people: 4,
          companies: 3,
          documents: 4,
          places: 4,
        ),
        filter: const DirectoryFilter(
          tags: [],
          expiring: false,
          hasOpenItems: false,
        ),
        sort: DirectorySort.name,
        filterOptions: [],
        sections: [],
        suggestions: [],
        expiringCount: 0,
      );

  /// The companies tab filtered by [query].
  static DirectoryView companies(String query, List<DirectoryItem> items) =>
      DirectoryView(
        tab: DirectoryTab.companies,
        query: query,
        items: items,
        counts: const DirectoryCounts(
          people: 4,
          companies: 3,
          documents: 4,
          places: 4,
        ),
        filter: const DirectoryFilter(
          tags: [],
          expiring: false,
          hasOpenItems: false,
        ),
        sort: DirectorySort.name,
        filterOptions: [],
        sections: [],
        suggestions: [],
        expiringCount: 0,
      );
}
