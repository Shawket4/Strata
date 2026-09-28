// Fixtures of the notes package: the "sales" folder and the "Pricing
// experiments" note of design/SCREEN_SPEC.md (NoteExpanded), plus the
// Arabic "تجارب التسعير — ملخص" note (NoteExpandedDarkAr). Hints are the
// spans the core emits for the content (exact UTF-16 offsets).
import 'package:strata_state/strata_state.dart';

EditorHint _hint(
  String content,
  HintKind kind,
  String text, {
  String? targetId,
  String? taskId,
  int level = 0,
}) {
  // Search the body only (after the frontmatter).
  final start = content.indexOf(text, content.indexOf('---\n', 4) + 4);
  if (start < 0) throw StateError('"$text" not in fixture');
  return EditorHint(
    kind: kind,
    start: start,
    end: start + text.length,
    targetId: targetId,
    taskId: taskId,
    level: level,
  );
}

EditorHint _frontmatter(String content) => EditorHint(
  kind: HintKind.frontmatter,
  start: 0,
  end: content.indexOf('---\n', 4) + 4,
  level: 0,
);

abstract final class NotesFixtures {
  static const String pricingId = 'n-pricing-experiments';
  static const String arabicId = 'n-pricing-summary-ar';
  static const String conflictOpId = '01J8ZQ9CONFL1CT0000000000A';

  static final NoteListItem pricing = NoteListItem(
    id: pricingId,
    title: 'Pricing experiments',
    path: 'notes/sales/Pricing experiments.md',
    kind: 'note',
    snippet:
        'Three experiments to run in Q4 before we lock the Subscription '
        'tiers. Ahmed Samir runs the Acme pilot.',
    tags: const ['pricing', 'q4'],
    updatedAt: DateTime.utc(2026, 9, 27, 11, 31),
    pendingSync: false,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: '14:31',
    linkCount: 0,
    highlights: [],
  );

  static final NoteListItem weekly = NoteListItem(
    id: 'n-weekly-invoicing-proposal',
    title: 'Weekly invoicing proposal',
    path: 'notes/sales/Weekly invoicing proposal.md',
    kind: 'note',
    snippet:
        'Acme asked to move from monthly to weekly invoicing starting '
        'October.',
    tags: const ['invoicing'],
    updatedAt: DateTime.utc(2026, 9, 27, 10, 2),
    pendingSync: true,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: '13:02',
    linkCount: 0,
    highlights: [],
  );

  static final NoteListItem arabic = NoteListItem(
    id: arabicId,
    title: 'تجارب التسعير — ملخص',
    path: 'notes/sales/تجارب التسعير — ملخص.md',
    kind: 'note',
    snippet: 'ملخص سريع لتجارب التسعير اللي هنعملها في الربع الرابع.',
    tags: const ['pricing'],
    updatedAt: DateTime.utc(2026, 9, 27, 9, 15),
    pendingSync: false,
    titleDir: TextDir.rtl,
    snippetDir: TextDir.rtl,
    updatedLabel: '12:15',
    linkCount: 0,
    highlights: [],
  );

  static final NoteListItem churn = NoteListItem(
    id: 'n-churn-notes',
    title: 'Churn notes',
    path: 'notes/sales/Churn notes.md',
    kind: 'note',
    snippet:
        'Customers past 12 months churn 40% less than first-year accounts.',
    tags: const ['churn'],
    updatedAt: DateTime.utc(2026, 9, 26, 8),
    pendingSync: false,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: 'Sat',
    linkCount: 0,
    highlights: [],
  );

  static final NotesListView sales = NotesListView(
    folder: 'notes/sales',
    folders: const [
      FolderItem(path: 'notes/sales/archive', name: 'archive', noteCount: 12),
    ],
    notes: [pricing, weekly, arabic, churn],
    breadcrumb: const [
      FolderItem(path: 'notes', name: 'notes', noteCount: 96),
      FolderItem(path: 'notes/sales', name: 'sales', noteCount: 7),
    ],
    noteCount: 7,
  );

  static const NotesListView emptyFolder = NotesListView(
    folder: 'notes/ops',
    folders: [],
    notes: [],
    breadcrumb: [],
    noteCount: 0,
  );

  static const RelationChip contradicts = RelationChip(
    relType: 'contradicts',
    target: EntityRef(id: 'n-discount-policy', title: 'Discount policy'),
    by: 'ai',
    confidence: 0.72,
    reason: 'States a flat 10% discount, while target caps discounts at 3%.',
    relLabel: 'contradicts',
    createdLabel: '14:05',
    citations: [
      Citation(
        noteId: pricingId,
        target: 'Pricing experiments',
        anchor: 'a1b2',
      ),
      Citation(
        noteId: 'n-discount-policy',
        target: 'Discount policy',
        anchor: 'cap',
      ),
    ],
  );

  static const RelationChip partOf = RelationChip(
    relType: 'part-of',
    target: EntityRef(id: 'n-subscription-tiers', title: 'Subscription tiers'),
    by: 'user',
    relLabel: 'part of',
    citations: [],
  );

  static const RelationChip related = RelationChip(
    relType: 'related',
    target: EntityRef(id: 'n-churn-notes', title: 'Churn notes'),
    by: 'ai',
    confidence: 0.81,
    reason: 'Both discuss retention of customers past 12 months.',
    relLabel: 'related',
    citations: [],
  );

  static const RelationChip concept = RelationChip(
    relType: 'concepts',
    target: EntityRef(id: 'c-pricing', title: 'Pricing'),
    by: 'ai',
    confidence: 0.9,
    relLabel: 'concept',
    citations: [],
  );

  static const List<BacklinkGroup> backlinks = [
    BacklinkGroup(
      kind: 'supports',
      items: [
        BacklinkItem(
          noteId: 'n-call-2026-09-12-acme',
          title: 'Call 2026-09-12 — Acme',
          titleDir: TextDir.ltr,
          snippet: 'Ahmed asked whether annual prepay gets a discount.',
          snippetDir: TextDir.ltr,
          by: 'user',
        ),
        BacklinkItem(
          noteId: 'n-onboarding-v2',
          title: 'Onboarding checklist v2',
          titleDir: TextDir.ltr,
          snippet: 'Show plan pricing on day one of the trial.',
          snippetDir: TextDir.ltr,
          by: 'ai',
          confidence: 0.77,
        ),
      ],
      label: 'supports',
    ),
    BacklinkGroup(
      kind: 'follows-up',
      items: [
        BacklinkItem(
          noteId: 'n-weekly-invoicing-proposal',
          title: 'Weekly invoicing proposal',
          titleDir: TextDir.ltr,
          snippet: 'Pair weekly invoicing with the annual prepay test.',
          snippetDir: TextDir.ltr,
        ),
      ],
      label: 'follows up',
    ),
    BacklinkGroup(
      kind: 'link',
      items: [
        BacklinkItem(
          noteId: 'n-q4-hiring',
          title: 'Q4 hiring plan',
          titleDir: TextDir.ltr,
          snippetDir: TextDir.ltr,
        ),
      ],
      label: 'body links',
    ),
  ];

  static const String pricingContent =
      '---\n'
      'id: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\n'
      'tags: [pricing, q4]\n'
      'part-of: ["[[Subscription tiers]]"]\n'
      '---\n'
      'Three experiments to run in Q4 before we lock the '
      '[[Subscription tiers]]. [[Ahmed Samir]] runs the Acme pilot.\n'
      '\n'
      '## Hypotheses\n'
      '- Annual prepay with 2 months free lifts conversion for teams of 5+.\n'
      '- Launch offer: a **flat 10% discount** on the first annual plan. '
      '^a1b2\n'
      '\n'
      '## Next steps\n'
      '- [ ] Draft two pricing page variants #pricing ^t-01j9p1\n';

  static final List<EditorHint> pricingHints = [
    _frontmatter(pricingContent),
    _hint(
      pricingContent,
      HintKind.wikiLink,
      '[[Subscription tiers]]',
      targetId: 'n-subscription-tiers',
    ),
    _hint(
      pricingContent,
      HintKind.wikiLink,
      '[[Ahmed Samir]]',
      targetId: 'p-ahmed-samir',
    ),
    _hint(pricingContent, HintKind.heading, '## Hypotheses', level: 2),
    _hint(pricingContent, HintKind.bold, '**flat 10% discount**'),
    _hint(pricingContent, HintKind.blockId, '^a1b2'),
    _hint(pricingContent, HintKind.heading, '## Next steps', level: 2),
    _hint(
      pricingContent,
      HintKind.taskLine,
      '- [ ] Draft two pricing page variants #pricing ^t-01j9p1',
      taskId: 't-01j9p1',
    ),
    _hint(pricingContent, HintKind.tag, '#pricing'),
    _hint(pricingContent, HintKind.blockId, '^t-01j9p1'),
  ];

  static const TaskItem task = TaskItem(
    id: 't-01j9p1',
    noteId: pricingId,
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

  /// The version history (newest first).
  static final List<HistoryEntry> history = [
    HistoryEntry(
      commit: 'c7',
      versionLabel: 'v7',
      message: 'user: edit Hypotheses',
      author: 'user',
      at: DateTime.utc(2026, 9, 27, 11, 31),
      atLabel: 'today 14:31',
      canRevert: false,
    ),
    HistoryEntry(
      commit: 'c6',
      versionLabel: 'v6',
      message: 'ai: contradicts [[Discount policy]]',
      author: 'ai',
      at: DateTime.utc(2026, 9, 27, 11, 5),
      atLabel: 'today 14:05',
      canRevert: true,
    ),
    HistoryEntry(
      commit: 'c5',
      versionLabel: 'v5',
      message: 'user: add Next steps',
      author: 'user',
      at: DateTime.utc(2026, 9, 26, 15, 20),
      atLabel: 'Sat 26 Sep 18:20',
      canRevert: true,
    ),
  ];

  static const NoteSyncState conflictSync = NoteSyncState(
    kind: NoteSyncKind.conflict,
    pendingOps: 1,
    conflictOpId: conflictOpId,
    label: 'Conflict',
  );

  static const NoteSyncState duplicateSync = NoteSyncState(
    kind: NoteSyncKind.duplicate,
    pendingOps: 1,
    duplicateOpId: duplicateOpId,
    label: 'Already exists?',
  );

  static const String duplicateOpId = '01J8ZQ9DUPL1CATE000000000B';

  static NoteView pricingNote({
    NoteSyncState sync = const NoteSyncState(
      kind: NoteSyncKind.synced,
      pendingOps: 0,
      label: 'Saved',
    ),
    bool pinned = false,
    Availability history = Availability.available,
    List<RelationChip> relations = const [
      contradicts,
      partOf,
      related,
      concept,
    ],
    List<BacklinkGroup> groups = backlinks,
  }) => NoteView(
    id: pricingId,
    path: 'notes/sales/Pricing experiments.md',
    title: 'Pricing experiments',
    kind: 'note',
    content: pricingContent,
    version: 'sha256:5f2c9e',
    properties: const [
      PropertyItem(key: 'id', values: ['01J8ZK3M4X7Q9W2E5R6T8Y0V1H']),
    ],
    relations: relations,
    backlinks: groups,
    tags: const ['pricing', 'q4'],
    tasks: const [task],
    hints: pricingHints,
    sync_: sync,
    history: history,
    titleDir: TextDir.ltr,
    contentVersion: 'v7-5f2c9e',
    versionLabel: 'v7',
    createdLabel: '18 Sep',
    editedLabel: 'today 14:31',
    editedBy: 'Shawket',
    wordCount: 214,
    backlinkCount: 6,
    historyEntries: history == Availability.available
        ? NotesFixtures.history
        : const [],
    pinned: pinned,
  );

  static const String arabicContent =
      '---\n'
      'id: 01J9AR4B1C0000000000000000\n'
      'lang: ar\n'
      '---\n'
      'ملخص سريع لتجارب التسعير اللي هنعملها في الربع الرابع، مبني على '
      '[[Pricing experiments]] ومراجعة سياسة الخصومات.\n'
      '\n'
      'اتكلمت مع [[أحمد سمير]] عن الـ annual prepay، وهو شايف إن شهرين '
      'مجانًا كفاية للـ teams من 5 أفراد أو أكتر.\n'
      '\n'
      '## الفرضيات\n'
      '- خصم ولاء 5% على التجديد للعملاء اللي بقالهم أكتر من 12 شهر.\n'
      '- نجهز نسختين من صفحة الأسعار ونختبرهم أسبوعين. ^c4d5\n'
      '\n'
      'Next: A/B test على صفحة الأسعار لمدة أسبوعين.\n';

  static final List<EditorHint> arabicHints = [
    _frontmatter(arabicContent),
    _hint(
      arabicContent,
      HintKind.wikiLink,
      '[[Pricing experiments]]',
      targetId: pricingId,
    ),
    _hint(arabicContent, HintKind.wikiLink, '[[أحمد سمير]]'),
    _hint(arabicContent, HintKind.heading, '## الفرضيات', level: 2),
    _hint(arabicContent, HintKind.blockId, '^c4d5'),
    for (final line in [
      'ملخص سريع لتجارب التسعير',
      'اتكلمت مع',
      '## الفرضيات',
      '- خصم ولاء',
      '- نجهز نسختين',
    ])
      _hint(arabicContent, HintKind.rtlLine, line),
    _hint(arabicContent, HintKind.ltrLine, 'Next: A/B test'),
  ];

  static final NoteView arabicNote = NoteView(
    id: arabicId,
    path: 'notes/sales/تجارب التسعير — ملخص.md',
    title: 'تجارب التسعير — ملخص',
    kind: 'note',
    content: arabicContent,
    version: 'sha256:a4b2',
    properties: const [
      PropertyItem(key: 'lang', values: ['ar']),
    ],
    relations: const [
      RelationChip(
        relType: 'follows-up',
        target: EntityRef(id: pricingId, title: 'Pricing experiments'),
        by: 'user',
        relLabel: 'follows up',
        citations: [],
      ),
      RelationChip(
        relType: 'related',
        target: EntityRef(
          id: 'n-subscription-tiers',
          title: 'Subscription tiers',
        ),
        by: 'ai',
        confidence: 0.81,
        reason: 'Both describe the loyalty tier.',
        relLabel: 'related',
        citations: [],
      ),
    ],
    backlinks: const [
      BacklinkGroup(
        kind: 'related',
        items: [
          BacklinkItem(
            noteId: pricingId,
            title: 'Pricing experiments',
            titleDir: TextDir.ltr,
            snippetDir: TextDir.ltr,
          ),
        ],
        label: 'related',
      ),
    ],
    tags: const ['pricing'],
    tasks: const [],
    hints: arabicHints,
    sync_: const NoteSyncState(
      kind: NoteSyncKind.synced,
      pendingOps: 0,
      label: 'Saved',
    ),
    history: Availability.available,
    titleDir: TextDir.rtl,
    contentVersion: '',
    versionLabel: 'v3',
    wordCount: 58,
    backlinkCount: 1,
    historyEntries: [],
    pinned: false,
  );
}
