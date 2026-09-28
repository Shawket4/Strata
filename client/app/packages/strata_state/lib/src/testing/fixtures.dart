import 'dart:typed_data';

import 'package:strata_bridge/strata_bridge.dart';

/// Shared sample data for every feature's widget and golden tests: one
/// fixture (or more, for distinct states) per generated view-model type,
/// with the realistic content of `design/SCREEN_SPEC.md` "Sample content"
/// (today is Sunday 27 Sep 2026, owner Shawket, Cairo time = UTC+3).
///
/// Each fixture is a single instance (`static final`), so equality holds
/// across uses: frb types compare lists by identity. Build variants with the
/// generated constructors, reusing these parts.
abstract final class StrataFixtures {
  // ---------------------------------------------------------------------------
  // Shared values
  // ---------------------------------------------------------------------------

  /// "Now" in the sample world: Sun 27 Sep 2026, 14:32 Cairo.
  static final DateTime now = DateTime.utc(2026, 9, 27, 11, 32);

  /// The op / item ID the fake's intents answer with.
  static const String opId = '01J8ZQ4M6T3K9V2B7X5N1C0RDA';

  /// Server URL of the sample install.
  static const String serverUrl = 'https://strata.example.com';

  // ---------------------------------------------------------------------------
  // References and citations
  // ---------------------------------------------------------------------------

  /// Ahmed Samir (person).
  static const EntityRef ahmedSamirRef = EntityRef(
    id: 'p-ahmed-samir',
    title: 'Ahmed Samir',
  );

  /// Acme Logistics (company).
  static const EntityRef acmeLogisticsRef = EntityRef(
    id: 'c-acme-logistics',
    title: 'Acme Logistics',
  );

  /// Watanya (company).
  static const EntityRef watanyaRef = EntityRef(
    id: 'c-watanya',
    title: 'Watanya',
  );

  /// Shady (person), last holder of the Watanya contract.
  static const EntityRef shadyRef = EntityRef(id: 'p-shady', title: 'Shady');

  /// The Watanya contract (document).
  static const EntityRef watanyaContractRef = EntityRef(
    id: 'd-watanya-contract',
    title: 'Watanya contract',
  );

  /// Nasr City office (place).
  static const EntityRef nasrCityOfficeRef = EntityRef(
    id: 'pl-nasr-city-office',
    title: 'Nasr City office',
  );

  /// Safe — Nasr City office (place).
  static const EntityRef safeRef = EntityRef(
    id: 'pl-nasr-city-safe',
    title: 'Safe — Nasr City office',
  );

  /// A citation of the "Call 2026-09-12 — Acme" note, block `a1b2`.
  static const Citation citation = Citation(
    noteId: 'n-call-2026-09-12-acme',
    target: 'Call 2026-09-12 — Acme',
    anchor: 'a1b2',
  );

  /// An Insights bullet of Ahmed Samir's page.
  static const CitedBullet citedBullet = CitedBullet(
    text: 'Prefers weekly invoicing',
    citations: [citation],
    dir: TextDir.ltr,
  );

  /// A property row.
  static const PropertyItem propertyItem = PropertyItem(
    key: 'role',
    values: ['Operations manager'],
  );

  /// An outgoing relation proposed by the AI.
  static const RelationChip relationChip = RelationChip(
    relType: 'works-at',
    target: acmeLogisticsRef,
    by: 'ai',
    confidence: 0.94,
    reason: 'Ahmed is described as Acme’s operations manager in the call note.',
    relLabel: '',
    citations: [],
  );

  // ---------------------------------------------------------------------------
  // Session and account
  // ---------------------------------------------------------------------------

  /// The core configuration of a Linux desktop install.
  static const CoreConfig coreConfig = CoreConfig(
    appDataDir: '/home/shawket/.local/share/strata',
    platform: Platform.linux,
    defaultDeviceName: 'shawket-laptop',
    defaultServerUrl: serverUrl,
  );

  /// Shawket's account.
  static const AccountSummary accountSummary = AccountSummary(
    userId: 'u-shawket',
    username: 'shawket',
    displayName: 'Shawket',
    role: 'admin',
    isAdmin: true,
    serverUrl: serverUrl,
    timezone: 'Africa/Cairo',
    uiLanguage: 'en',
    initials: '',
  );

  /// An account with local data on this device.
  static const KnownAccountItem knownAccountItem = KnownAccountItem(
    userId: 'u-shawket',
    username: 'shawket',
    displayName: 'Shawket',
    serverUrl: serverUrl,
    initials: '',
  );

  /// Signed in and usable.
  static const SessionState sessionActive = SessionState(
    kind: SessionKind.active,
    account: accountSummary,
    knownAccounts: [knownAccountItem],
    serverUrl: serverUrl,
    deviceName: 'shawket-laptop',
    unsyncedOps: 0,
  );

  /// The login screen, with one known account.
  static const SessionState sessionSignedOut = SessionState(
    kind: SessionKind.signedOut,
    knownAccounts: [knownAccountItem],
    serverUrl: serverUrl,
    deviceName: 'shawket-laptop',
    unsyncedOps: 0,
  );

  /// Scheduled for deletion: 12 days left, 2 ops never synced.
  static final SessionState sessionDeletionPending = SessionState(
    kind: SessionKind.deletionPending,
    account: accountSummary,
    knownAccounts: const [knownAccountItem],
    serverUrl: serverUrl,
    deviceName: 'shawket-laptop',
    unsyncedOps: 2,
    deletionAt: DateTime.utc(2026, 10, 9, 21),
    daysRemaining: 12,
  );

  /// A sign-in form as submitted.
  static const SignInRequest signInRequest = SignInRequest(
    serverUrl: serverUrl,
    username: 'shawket',
    password: 'correct horse battery staple',
    deviceName: 'shawket-laptop',
  );

  /// A sign-up form as submitted.
  static const SignUpRequest signUpRequest = SignUpRequest(
    serverUrl: serverUrl,
    username: 'mona',
    password: 'nile-freight-2026',
    displayName: 'Mona Hassan',
  );

  /// Registered, waiting for approval.
  static const SignUpOutcome signUpOutcome = SignUpOutcome(username: 'mona');

  /// Signed out.
  static const SignOutOutcome signOutOutcome = SignOutOutcome(
    signedOut: true,
    unsyncedOps: 0,
  );

  /// Not signed out: 3 ops are unsynced.
  static const SignOutOutcome signOutNeedsConfirmation = SignOutOutcome(
    signedOut: false,
    unsyncedOps: 3,
  );

  /// A typed core error.
  static const CoreFailure coreFailure = CoreFailure(
    code: 'pending_changes',
    messageKey: 'error.pending_changes',
    count: 3,
  );

  // ---------------------------------------------------------------------------
  // Notes
  // ---------------------------------------------------------------------------

  /// "Pricing experiments" in notes/sales.
  static final NoteListItem noteListItem = NoteListItem(
    id: 'n-pricing-experiments',
    title: 'Pricing experiments',
    path: 'notes/sales/Pricing experiments.md',
    kind: 'note',
    snippet:
        'Test a 5% loyalty discount on renewals for customers > 12 months.',
    tags: const ['pricing', 'sales'],
    updatedAt: DateTime.utc(2026, 9, 27, 9, 5),
    pendingSync: false,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: '',
    linkCount: 0,
    highlights: [],
  );

  /// "Call 2026-09-12 — Acme", mixed Arabic/English, not yet synced.
  static final NoteListItem noteListItemMixed = NoteListItem(
    id: 'n-call-2026-09-12-acme',
    title: 'Call 2026-09-12 — Acme',
    path: 'notes/clients/acme/Call 2026-09-12 — Acme.md',
    kind: 'note',
    snippet: 'أحمد طلب invoicing أسبوعي بدل شهري ابتداءً من أكتوبر.',
    tags: const ['acme', 'invoicing'],
    updatedAt: DateTime.utc(2026, 9, 26, 16, 40),
    pendingSync: true,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: '',
    linkCount: 0,
    highlights: [],
  );

  /// "تجارب التسعير — ملخص" (Arabic title).
  static final NoteListItem noteListItemArabic = NoteListItem(
    id: 'n-pricing-summary-ar',
    title: 'تجارب التسعير — ملخص',
    path: 'notes/sales/تجارب التسعير — ملخص.md',
    kind: 'note',
    snippet: 'ملخص نتائج تجارب التسعير للربع الثالث.',
    tags: const ['pricing'],
    updatedAt: DateTime.utc(2026, 9, 25, 12),
    pendingSync: false,
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    updatedLabel: '',
    linkCount: 0,
    highlights: [],
  );

  /// A backlink source.
  static const BacklinkItem backlinkItem = BacklinkItem(
    noteId: 'n-weekly-invoicing-proposal',
    title: 'Weekly invoicing proposal',
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
  );

  /// Backlinks of type `follows-up`.
  static const BacklinkGroup backlinkGroup = BacklinkGroup(
    kind: 'follows-up',
    items: [backlinkItem],
    label: '',
  );

  /// A heading span.
  static const EditorHint editorHint = EditorHint(
    kind: HintKind.heading,
    start: 0,
    end: 24,
    level: 0,
  );

  /// The hints of [noteView]'s content.
  static const List<EditorHint> editorHints = [
    editorHint,
    EditorHint(kind: HintKind.wikiLink, start: 26, end: 41, level: 0),
  ];

  /// A note with local changes waiting to sync.
  static const NoteSyncState noteSyncState = NoteSyncState(
    kind: NoteSyncKind.pending,
    pendingOps: 1,
    label: '',
  );

  /// The "Call 2026-09-12 — Acme" note.
  static final NoteView noteView = NoteView(
    id: 'n-call-2026-09-12-acme',
    path: 'notes/clients/acme/Call 2026-09-12 — Acme.md',
    title: 'Call 2026-09-12 — Acme',
    kind: 'note',
    content:
        '# Call 2026-09-12 — Acme\n'
        '[[Ahmed Samir]] from [[Acme Logistics]]:\n'
        'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري ابتداءً من '
        'أكتوبر ^a1b2\n',
    version: 'sha256:5f2c9e',
    properties: const [
      PropertyItem(key: 'people', values: ['Ahmed Samir']),
      PropertyItem(key: 'companies', values: ['Acme Logistics']),
    ],
    relations: const [
      RelationChip(
        relType: 'about',
        target: acmeLogisticsRef,
        by: 'user',
        relLabel: '',
        citations: [],
      ),
    ],
    backlinks: const [backlinkGroup],
    tags: const ['acme', 'invoicing'],
    tasks: [taskAhmedProposal],
    hints: editorHints,
    sync_: noteSyncState,
    history: Availability.available,
    titleDir: TextDir.ltr,
    contentVersion: '',
    wordCount: 0,
    backlinkCount: 0,
    historyEntries: [],
    pinned: false,
  );

  /// The note screen of [noteView].
  static final NoteScreen noteScreen = NoteScreen(
    id: 'n-call-2026-09-12-acme',
    note: noteView,
  );

  /// A folder row.
  static const FolderItem folderItem = FolderItem(
    path: 'notes/clients',
    name: 'clients',
    noteCount: 4,
  );

  /// The `notes` folder.
  static final NotesListView notesListView = NotesListView(
    folder: 'notes',
    folders: const [
      folderItem,
      FolderItem(path: 'notes/sales', name: 'sales', noteCount: 6),
    ],
    notes: [noteListItem, noteListItemArabic],
    breadcrumb: [],
    noteCount: 0,
  );

  // ---------------------------------------------------------------------------
  // Tasks
  // ---------------------------------------------------------------------------

  /// A reminder at 09:00 Cairo on Thu 1 Oct 2026.
  static final ReminderItem reminderItem = ReminderItem(
    local: '2026-10-01 09:00',
    at: DateTime.utc(2026, 10, 1, 6),
    localAt: StrataFixtures.now,
    timeLabel: '',
    offsetLabel: '',
  );

  /// "Make Watanya's ETA invoice" — monthly on the 1st, due Thu 1 Oct.
  static final TaskItem taskWatanyaEtaInvoice = TaskItem(
    id: 't-watanya-eta',
    noteId: 'n-tasks',
    noteTitle: 'Tasks',
    description: "Make Watanya's ETA invoice",
    state: TaskState.open,
    priority: 'normal',
    due: DateTime.utc(2026, 10),
    recurrence: 'every month on the 1st',
    recurrenceUnderstood: true,
    reminders: [reminderItem],
    links: const [watanyaRef],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// "Petrol Arrows invoice" — weekly on Sunday, due today 10:00.
  static final TaskItem taskPetrolArrowsInvoice = TaskItem(
    id: 't-petrol-arrows',
    noteId: 'n-tasks',
    noteTitle: 'Tasks',
    description: 'Petrol Arrows invoice',
    state: TaskState.open,
    priority: 'normal',
    due: DateTime.utc(2026, 9, 27),
    recurrence: 'every week on Sunday',
    recurrenceUnderstood: true,
    reminders: [
      ReminderItem(
        local: '2026-09-27 10:00',
        at: DateTime.utc(2026, 9, 27, 7),
        localAt: StrataFixtures.now,
        timeLabel: '',
        offsetLabel: '',
      ),
    ],
    links: const [EntityRef(id: 'c-petrol-arrows', title: 'Petrol Arrows')],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// "Send weekly invoicing proposal to Ahmed" — due Tue 29 Sep.
  static final TaskItem taskAhmedProposal = TaskItem(
    id: 't-ahmed-proposal',
    noteId: 'n-call-2026-09-12-acme',
    noteTitle: 'Call 2026-09-12 — Acme',
    description: 'Send weekly invoicing proposal to Ahmed',
    state: TaskState.open,
    priority: 'high',
    due: DateTime.utc(2026, 9, 29),
    recurrenceUnderstood: true,
    reminders: const [],
    links: const [ahmedSamirRef, acmeLogisticsRef],
    pendingSync: true,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// "Pay Nile Freight September invoice" — overdue since Thu 24 Sep.
  static final TaskItem taskOverdue = TaskItem(
    id: 't-nile-freight',
    noteId: 'n-tasks',
    noteTitle: 'Tasks',
    description: 'Pay Nile Freight September invoice',
    state: TaskState.open,
    priority: 'normal',
    due: DateTime.utc(2026, 9, 24),
    recurrenceUnderstood: true,
    reminders: const [],
    links: const [EntityRef(id: 'c-nile-freight', title: 'Nile Freight')],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// "Make Watanya's ETA invoice" done on 1 Sep (history).
  static final TaskItem taskWatanyaDoneSeptember = TaskItem(
    id: 't-watanya-eta-2026-09',
    noteId: 'n-tasks',
    noteTitle: 'Tasks',
    description: "Make Watanya's ETA invoice",
    state: TaskState.done,
    priority: 'normal',
    due: DateTime.utc(2026, 9),
    done: DateTime.utc(2026, 9),
    recurrence: 'every month on the 1st',
    recurrenceUnderstood: true,
    reminders: const [],
    links: const [watanyaRef],
    pendingSync: false,
    descriptionDir: TextDir.ltr,
    notePath: '',
    lineNumber: 0,
    isOverdue: false,
  );

  /// Home's task sections.
  static final TaskSections taskSections = TaskSections(
    overdue: [taskOverdue],
    today: [taskPetrolArrowsInvoice],
    upcoming: [taskAhmedProposal, taskWatanyaEtaInvoice],
    recurring: [taskPetrolArrowsInvoice, taskWatanyaEtaInvoice],
    noDate: const [],
    upcomingGroups: [],
    todayCount: 0,
  );

  /// The Tasks destination.
  static final TasksView tasksView = TasksView(
    sections: taskSections,
    done: [taskWatanyaDoneSeptember],
    openCount: 0,
    doneThisWeek: 0,
    doneThisWeekLabel: '',
    notesWithTasks: 0,
  );

  /// Task detail of the Watanya ETA invoice.
  static final TaskScreen taskScreen = TaskScreen(
    id: 't-watanya-eta',
    task: taskWatanyaEtaInvoice,
    line:
        "- [ ] Make Watanya's ETA invoice [[Watanya]] 🔁 every month on the "
        '1st 📅 2026-10-01 ⏰ 2026-10-01 09:00 ^t-watanya-eta',
    history: [taskWatanyaDoneSeptember],
    locationLabel: '',
    recurrencePreview: [],
  );

  /// A new task as drafted in the task editor.
  static final TaskDraft taskDraft = TaskDraft(
    description: "Make Watanya's ETA invoice",
    due: DateTime.utc(2026, 10),
    recurrence: 'every month on the 1st',
    reminders: [DateTime.utc(2026, 10, 1, 9)],
  );

  /// An edit of the Ahmed proposal task: due moved to Wed 30 Sep.
  static final TaskPatch taskPatch = TaskPatch(
    due: DateTime.utc(2026, 9, 30),
    clearDue: false,
    clearRecurrence: false,
  );

  // ---------------------------------------------------------------------------
  // Sync
  // ---------------------------------------------------------------------------

  /// Idle sync activity.
  static const SyncActivity syncActivity = SyncActivity(
    phase: SyncPhase.idle,
    pagesDone: 0,
    ops: 0,
    opsDone: 0,
    opsTotal: 0,
    pulled: 0,
  );

  /// "Synced · 14:32".
  static final SyncPill syncPill = SyncPill(
    connectivity: Connectivity.online,
    activity: syncActivity,
    pendingOps: 0,
    conflicts: 0,
    duplicates: 0,
    lastSyncAt: now,
    display: SyncPillKind.synced,
    progressDone: 0,
    progressTotal: 0,
    label: '',
  );

  /// "Offline · 3 changes queued".
  static final SyncPill syncPillOffline = SyncPill(
    connectivity: Connectivity.offline,
    activity: syncActivity,
    pendingOps: 3,
    conflicts: 0,
    duplicates: 0,
    lastSyncAt: DateTime.utc(2026, 9, 27, 8, 15),
    display: SyncPillKind.synced,
    progressDone: 0,
    progressTotal: 0,
    label: '',
  );

  /// "Syncing 12/40".
  static final SyncPill syncPillSyncing = SyncPill(
    connectivity: Connectivity.online,
    activity: const SyncActivity(
      phase: SyncPhase.pushing,
      pagesDone: 0,
      ops: 40,
      opsDone: 0,
      opsTotal: 0,
      pulled: 0,
    ),
    pendingOps: 28,
    conflicts: 0,
    duplicates: 0,
    lastSyncAt: DateTime.utc(2026, 9, 27, 8, 15),
    display: SyncPillKind.synced,
    progressDone: 0,
    progressTotal: 0,
    label: '',
  );

  /// "1 conflict".
  static final SyncPill syncPillConflict = SyncPill(
    connectivity: Connectivity.online,
    activity: syncActivity,
    pendingOps: 1,
    conflicts: 1,
    duplicates: 0,
    lastSyncAt: now,
    display: SyncPillKind.synced,
    progressDone: 0,
    progressTotal: 0,
    label: '',
  );

  /// A queued op.
  static final OutboxItem outboxItem = OutboxItem(
    opId: '01J8ZQ3W1E5H8S0Y4R6P2F9GKB',
    kind: 'note.update',
    title: 'Call 2026-09-12 — Acme',
    status: OutboxStatus.pending,
    attempts: 0,
    created: DateTime.utc(2026, 9, 26, 16, 40),
    detail: '',
    detailDir: TextDir.ltr,
    createdLabel: '',
  );

  /// A conflict row.
  static final ConflictItem conflictItem = ConflictItem(
    opId: '01J8ZQ2A7C4D6F8G0H2J4K6M8N',
    noteId: 'n-discount-policy',
    title: 'Discount policy',
    created: DateTime.utc(2026, 9, 27, 10, 2),
    createdLabel: '',
  );

  /// A rolled-back op.
  static const RejectionItem rejectionItem = RejectionItem(
    opId: '01J8ZQ1B2C3D4E5F6G7H8J9K0M',
    kind: 'note.move',
    problemType: 'path-exists',
    messageKey: 'error.path-exists',
  );

  /// Sync status with one of each list.
  static final SyncStatusView syncStatusView = SyncStatusView(
    pill: syncPillConflict,
    bootstrapComplete: true,
    outbox: [outboxItem],
    conflicts: [conflictItem],
    rejections: const [rejectionItem],
    paused: false,
    log: [],
  );

  /// One conflicting hunk.
  static const ConflictHunkView conflictHunkView = ConflictHunkView(
    id: 0,
    location: 'body:3',
    kind: 'body',
    base: 'Discounts are capped at 3%.\n',
    ours: 'Discounts are capped at 5% for loyal customers.\n',
    theirs: 'Discounts are capped at 4%.\n',
    locationLabel: '',
    allowedChoices: [],
  );

  /// The conflict on "Discount policy".
  static const ConflictDetail conflictDetail = ConflictDetail(
    noteId: 'n-discount-policy',
    title: 'Discount policy',
    base: '# Discount policy\n\nDiscounts are capped at 3%.\n',
    local:
        '# Discount policy\n\nDiscounts are capped at 5% for loyal '
        'customers.\n',
    server: '# Discount policy\n\nDiscounts are capped at 4%.\n',
    mergedPreview:
        '# Discount policy\n\n<<<<<<< mine\nDiscounts are capped at 5% for '
        'loyal customers.\n=======\nDiscounts are capped at 4%.\n>>>>>>> '
        'server\n',
    mergeClean: false,
    hunks: [conflictHunkView],
    path: '',
    localOriginLabel: '',
    serverOriginLabel: '',
    baseLines: [],
    localLines: [],
    serverLines: [],
  );

  /// The conflict screen of [conflictDetail].
  static const ConflictScreen conflictScreen = ConflictScreen(
    opId: '01J8ZQ2A7C4D6F8G0H2J4K6M8N',
    conflict: conflictDetail,
  );

  /// Keep mine for hunk 0.
  static const HunkChoice hunkChoice = HunkChoice(
    hunk: 0,
    choice: HunkChoiceKind.ours,
  );

  /// Resolve by hunks.
  static const ConflictResolution conflictResolution = ConflictResolution(
    kind: ResolutionKind.hunks,
    choices: [hunkChoice],
  );

  // ---------------------------------------------------------------------------
  // Duplicates
  // ---------------------------------------------------------------------------

  /// "Already exists: Make Watanya's ETA invoice" (near, 0.91).
  static const CandidateItem candidateItem = CandidateItem(
    id: 't-watanya-eta',
    kind: 'task',
    title: "Make Watanya's ETA invoice",
    snippet: 'monthly on the 1st · next Thu 1 Oct',
    matchLevel: 'near',
    score: 0.91,
    reason: '',
  );

  /// Created.
  static const CreateOutcome createOutcomeCreated = CreateOutcome(
    id: 'n-weekly-invoicing-request',
    candidates: [],
  );

  /// Not created: a near duplicate exists.
  static const CreateOutcome createOutcomeDuplicate = CreateOutcome(
    candidates: [candidateItem],
  );

  /// The prompt for "remind me to make watanya's invoice".
  static const DuplicatePrompt duplicatePrompt = DuplicatePrompt(
    opId: '01J8ZQ5N7P9R1T3V5X7Z9B1D3F',
    kind: 'task',
    title: "remind me to make watanya's invoice",
    candidates: [candidateItem],
  );

  /// Open prompts.
  static const DuplicatePromptsView duplicatePromptsView = DuplicatePromptsView(
    prompts: [duplicatePrompt],
  );

  // ---------------------------------------------------------------------------
  // Inbox
  // ---------------------------------------------------------------------------

  /// The filing proposal for the Acme capture.
  static const SuggestionDetail suggestionDetail = SuggestionDetail(
    kind: SuggestionKind.filing,
    title: 'Weekly invoicing request — Acme',
    folder: 'notes/clients/acme',
    tags: ['invoicing', 'acme'],
    mention: '',
    candidates: [ahmedSamirRef, acmeLogisticsRef],
    line: '',
    confidence: 0.88,
    duplicates: [],
    relType: '',
    reason: '',
    serverKind: '',
    documentChoices: [],
  );

  /// The filing suggestion of [inboxItem].
  static final SuggestionItem suggestionItem = SuggestionItem(
    id: 's-filing-acme',
    noteId: 'n-capture-acme',
    status: 'pending',
    detail: suggestionDetail,
    created: DateTime.utc(2026, 9, 27, 7, 12),
    pendingSync: false,
    createdLabel: '',
    sourceDir: TextDir.ltr,
    autoApplied: false,
    canAccept: false,
    needsYou: false,
    thread: [],
  );

  /// "Who is “بابا”?" — a link-or-create suggestion.
  static final SuggestionItem suggestionLinkOrCreate = SuggestionItem(
    id: 's-who-is-baba',
    noteId: 'n-capture-baba',
    status: 'pending',
    detail: const SuggestionDetail(
      kind: SuggestionKind.entityLinkOrCreate,
      title: '',
      folder: '',
      tags: [],
      mention: 'بابا',
      candidates: [],
      line: '',
      duplicates: [],
      relType: '',
      reason: '',
      serverKind: '',
      documentChoices: [],
    ),
    created: DateTime.utc(2026, 9, 27, 8),
    pendingSync: false,
    createdLabel: '',
    sourceDir: TextDir.ltr,
    autoApplied: false,
    canAccept: false,
    needsYou: false,
    thread: [],
  );

  /// The mixed-script Acme capture.
  static final InboxItem inboxItem = InboxItem(
    noteId: 'n-capture-acme',
    title: '2026-09-27 10-12',
    text:
        'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري ابتداءً من '
        'أكتوبر',
    created: DateTime.utc(2026, 9, 27, 7, 12),
    suggestions: [suggestionItem],
    pendingSync: false,
    textDir: TextDir.ltr,
    createdLabel: '',
    needsYou: false,
    ready: false,
    isDuplicate: false,
  );

  /// An English capture without suggestions yet.
  static final InboxItem inboxItemEnglish = InboxItem(
    noteId: 'n-capture-nile',
    title: '2026-09-27 09-40',
    text: 'Mona said Nile Freight rates go up 8% in November',
    created: DateTime.utc(2026, 9, 27, 6, 40),
    suggestions: const [],
    pendingSync: true,
    textDir: TextDir.ltr,
    createdLabel: '',
    needsYou: false,
    ready: false,
    isDuplicate: false,
  );

  /// The inbox.
  static final InboxView inboxView = InboxView(
    captures: [inboxItem, inboxItemEnglish],
    suggestions: [suggestionLinkOrCreate],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );

  // ---------------------------------------------------------------------------
  // Home
  // ---------------------------------------------------------------------------

  /// Home.
  static final HomeView homeView = HomeView(
    recentNotes: [noteListItem, noteListItemMixed, noteListItemArabic],
    inboxCount: 2,
    tasks: taskSections,
    sync_: syncPill,
    todayLabel: '',
    greeting: '',
    displayName: '',
    inboxPreview: [],
    needsYouCount: 0,
    contradictionsCount: 0,
    inboxSummary: '',
    aiActivity: Availability.available,
    aiActivityItems: [],
    aiActivityHeadline: '',
    openItems: Availability.available,
    openItemList: [],
    pinned: [],
  );

  // ---------------------------------------------------------------------------
  // Directory, entities, documents and places
  // ---------------------------------------------------------------------------

  /// Directory counts.
  static const DirectoryCounts directoryCounts = DirectoryCounts(
    people: 4,
    companies: 3,
    documents: 4,
    places: 6,
  );

  /// Ahmed Samir's row.
  static const DirectoryItem directoryItem = DirectoryItem(
    id: 'p-ahmed-samir',
    title: 'Ahmed Samir',
    subtitle: 'Operations manager, Acme Logistics',
    aliases: ['أحمد سمير', 'Ahmed Sameer'],
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

  /// The People tab.
  static const DirectoryView directoryView = DirectoryView(
    tab: DirectoryTab.people,
    query: '',
    items: [
      directoryItem,
      DirectoryItem(
        id: 'p-mona-hassan',
        title: 'Mona Hassan',
        subtitle: 'Finance lead, Nile Freight',
        aliases: ['منى حسن'],
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
      ),
    ],
    counts: directoryCounts,
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// The Watanya contract as listed on a page.
  static const DocumentBrief documentBrief = DocumentBrief(
    id: 'd-watanya-contract',
    title: 'Watanya contract',
    status: 'stored',
    location: safeRef,
    locationPath: [],
    expiringSoon: false,
    titleDir: TextDir.ltr,
  );

  /// "20 Sep 2026 — returned to the safe by Shady".
  static final CustodyItem custodyItem = CustodyItem(
    date: DateTime.utc(2026, 9, 20),
    kind: 'stored-at',
    document: watanyaContractRef,
    place: safeRef,
    person: shadyRef,
    citations: const [
      Citation(
        noteId: 'n-capture-watanya-safe',
        target: '2026-09-20 18-05',
        anchor: 'c7d8',
      ),
    ],
    by: '',
    sentenceKey: '',
    sentence: '',
    dateLabel: '',
    here: false,
  );

  /// The Watanya contract page.
  static final DocumentView documentView = DocumentView(
    id: 'd-watanya-contract',
    title: 'Watanya contract',
    aliases: const ['عقد وطنية'],
    docType: 'contract',
    copy: 'original',
    status: 'stored',
    expires: DateTime.utc(2027, 3, 31),
    location: const [nasrCityOfficeRef, safeRef],
    lastHolder: shadyRef,
    custody: [
      custodyItem,
      CustodyItem(
        date: DateTime.utc(2026, 9, 14),
        kind: 'handed-to',
        person: shadyRef,
        citations: const [citation],
        by: '',
        sentenceKey: '',
        sentence: '',
        dateLabel: '',
        here: false,
      ),
      CustodyItem(
        date: DateTime.utc(2026, 3, 2),
        kind: 'stored-at',
        place: safeRef,
        citations: const [citation],
        by: '',
        sentenceKey: '',
        sentence: '',
        dateLabel: '',
        here: false,
      ),
    ],
    copies: const [
      EntityRef(
        id: 'd-watanya-contract-copy',
        title: 'Watanya contract — copy',
      ),
    ],
    concerns: const [watanyaRef],
    titleDir: TextDir.ltr,
    path: '',
    pendingSync: false,
    expiringSoon: false,
    mentions: [],
    copyBriefs: [],
    userNotes: '',
  );

  /// Nasr City office (مكتب مدينة نصر).
  static final PlaceView placeView = PlaceView(
    id: 'pl-nasr-city-office',
    title: 'Nasr City office',
    aliases: const ['مكتب مدينة نصر'],
    breadcrumb: const [],
    subPlaces: const [
      safeRef,
      EntityRef(id: 'pl-nasr-city-cabinet-b', title: 'Cabinet B'),
    ],
    documents: const [
      documentBrief,
      DocumentBrief(
        id: 'd-petrol-arrows-register',
        title: 'Petrol Arrows commercial register',
        status: 'stored',
        location: EntityRef(id: 'pl-nasr-city-cabinet-b', title: 'Cabinet B'),
        locationPath: [],
        expiringSoon: false,
        titleDir: TextDir.ltr,
      ),
    ],
    recentMovements: [custodyItem],
    titleDir: TextDir.ltr,
    tree: [],
    outWithPeople: [],
    userNotes: '',
    path: '',
  );

  /// Ahmed Samir's page.
  static final EntityView entityView = EntityView(
    id: 'p-ahmed-samir',
    kind: 'person',
    title: 'Ahmed Samir',
    aliases: const ['أحمد سمير', 'Ahmed Sameer'],
    properties: const [
      propertyItem,
      PropertyItem(key: 'company', values: ['Acme Logistics']),
    ],
    summary:
        'Operations manager at Acme Logistics; main contact for invoicing '
        'and delivery schedules.',
    insights: const [citedBullet],
    openItems: const [
      CitedBullet(
        text: 'Send the weekly invoicing proposal',
        citations: [citation],
        dir: TextDir.ltr,
      ),
    ],
    timeline: [
      CitedBullet(
        text: 'Asked for weekly invoicing from October',
        date: DateTime.utc(2026, 9, 12),
        citations: const [citation],
        dir: TextDir.ltr,
      ),
    ],
    mentions: [noteListItemMixed],
    related: const [relationChip],
    documents: const [],
    pendingSync: false,
    titleDir: TextDir.ltr,
    initials: '',
    path: '',
    tags: [],
    userNotes: '',
    summaryCitations: [],
    openCount: 0,
    doneCount: 0,
    mentionCount: 0,
    summaryDir: TextDir.ltr,
  );

  /// Entity screen: Ahmed Samir.
  static final EntityScreen entityScreen = EntityScreen(
    id: 'p-ahmed-samir',
    kind: EntityPageKind.entity,
    entity: entityView,
  );

  /// Entity screen: the Watanya contract.
  static final EntityScreen entityScreenDocument = EntityScreen(
    id: 'd-watanya-contract',
    kind: EntityPageKind.document,
    document: documentView,
  );

  /// Entity screen: Nasr City office.
  static final EntityScreen entityScreenPlace = EntityScreen(
    id: 'pl-nasr-city-office',
    kind: EntityPageKind.place,
    place: placeView,
  );

  // ---------------------------------------------------------------------------
  // Graphs
  // ---------------------------------------------------------------------------

  /// The focused note of a local graph.
  static const GraphNode graphNode = GraphNode(
    id: 'p-ahmed-samir',
    title: 'Ahmed Samir',
    kind: GraphNodeKind.person,
    depth: 0,
    clusterId: 'k-clients',
    degree: 5,
    x: 0,
    y: 0,
    titleDir: TextDir.ltr,
    updatedLabel: '',
    labelRank: 0,
    isHub: false,
  );

  /// Ahmed works at Acme (AI, 0.94).
  static const GraphEdge graphEdge = GraphEdge(
    src: 'p-ahmed-samir',
    dst: 'c-acme-logistics',
    kind: 'relation:works-at',
    by: 'ai',
    confidence: 0.94,
    id: '',
    label: '',
  );

  /// The Clients cluster.
  static const ClusterLabel clusterLabel = ClusterLabel(
    id: 'k-clients',
    name: 'Clients',
    size: 12,
    x: 0,
    y: 0,
    hull: [],
    radius: 0,
  );

  static const List<GraphNode> _graphNodes = [
    graphNode,
    GraphNode(
      id: 'c-acme-logistics',
      title: 'Acme Logistics',
      kind: GraphNodeKind.company,
      depth: 1,
      clusterId: 'k-clients',
      degree: 7,
      x: 120,
      y: -40,
      titleDir: TextDir.ltr,
      updatedLabel: '',
      labelRank: 0,
      isHub: false,
    ),
    GraphNode(
      id: 'n-call-2026-09-12-acme',
      title: 'Call 2026-09-12 — Acme',
      kind: GraphNodeKind.note,
      depth: 1,
      clusterId: 'k-clients',
      degree: 3,
      x: -90,
      y: 80,
      titleDir: TextDir.ltr,
      updatedLabel: '',
      labelRank: 0,
      isHub: false,
    ),
  ];

  static const List<GraphEdge> _graphEdges = [
    graphEdge,
    GraphEdge(
      src: 'n-call-2026-09-12-acme',
      dst: 'p-ahmed-samir',
      kind: 'mention',
      id: '',
      label: '',
    ),
  ];

  /// Ahmed Samir's local mind map, depth 1.
  static const LocalGraphView localGraphView = LocalGraphView(
    center: 'p-ahmed-samir',
    found: true,
    depth: 1,
    nodes: _graphNodes,
    edges: _graphEdges,
    relationCount: 0,
    aiRelationCount: 0,
    relationLabel: '',
    saveLayout: Availability.available,
    proposeRelation: Availability.available,
  );

  /// The global map.
  static const GlobalGraphView globalGraphView = GlobalGraphView(
    nodes: _graphNodes,
    edges: _graphEdges,
    clusters: [clusterLabel],
    filter: GraphFilter(
      edgeKinds: [],
      nodeKinds: [],
      similarity: false,
      lens: GraphLens.notes,
      includeTags: false,
    ),
    edgeCounts: [],
    nodeCounts: [],
    neighbours: [],
    similarity: Availability.available,
  );

  // ---------------------------------------------------------------------------
  // Search and Ask
  // ---------------------------------------------------------------------------

  /// A search hit.
  static const SearchHit searchHit = SearchHit(
    noteId: 'n-pricing-experiments',
    title: 'Pricing experiments',
    path: 'notes/sales/Pricing experiments.md',
    kind: 'note',
    snippet: '… a 5% loyalty discount on renewals …',
    titleDir: TextDir.ltr,
    snippetDir: TextDir.ltr,
    highlights: [],
    score: 0,
  );

  /// Keyword search for "pricing".
  static const SearchView searchView = SearchView(
    query: 'pricing',
    mode: SearchMode.keyword,
    results: [
      searchHit,
      SearchHit(
        noteId: 'n-pricing-summary-ar',
        title: 'تجارب التسعير — ملخص',
        path: 'notes/sales/تجارب التسعير — ملخص.md',
        kind: 'note',
        snippet: 'ملخص نتائج تجارب pricing للربع الثالث',
        titleDir: TextDir.ltr,
        snippetDir: TextDir.ltr,
        highlights: [],
        score: 0,
      ),
    ],
    availability: Availability.available,
    availableModes: [],
  );

  /// An answer with a citation.
  static const AskMessage askMessage = AskMessage(
    role: 'assistant',
    text: 'Ahmed asked for weekly invoicing starting in October.',
    citations: [citation],
    id: '',
    streaming: false,
    spans: [],
    sources: [],
    scopeLabel: '',
    sourceCount: 0,
    createdLabel: '',
    dir: TextDir.ltr,
  );

  /// Ask (not available until `/ask` exists).
  static const AskView askView = AskView(
    availability: Availability.notYetAvailable,
    messages: [
      AskMessage(
        role: 'user',
        text: 'What did Ahmed ask for?',
        citations: [],
        id: '',
        streaming: false,
        spans: [],
        sources: [],
        scopeLabel: '',
        sourceCount: 0,
        createdLabel: '',
        dir: TextDir.ltr,
      ),
      askMessage,
    ],
    scopes: [],
    streaming: false,
  );

  // ---------------------------------------------------------------------------
  // Settings, admin, reminders
  // ---------------------------------------------------------------------------

  /// Reminders on, OS-scheduled.
  static const RemindersSetting remindersSetting = RemindersSetting(
    enabled: true,
    permission: NotificationPermission.granted,
    mode: NotificationMode.osScheduled,
    scheduled: 4,
    defaultTime: '09:00',
    snoozeMinutes: 0,
    quietEnabled: false,
    quietFrom: '',
    quietUntil: '',
  );

  /// Settings.
  static const SettingsView settingsView = SettingsView(
    account: accountSummary,
    reminders: remindersSetting,
    devices: Availability.available,
    ai: Availability.notYetAvailable,
    export_: Availability.available,
    integrity: Availability.notYetAvailable,
    admin: Availability.available,
    deviceList: [],
    integrityWarnings: [],
  );

  /// Mona Hassan, waiting for approval.
  static final AdminUserItem adminUserItem = AdminUserItem(
    id: 'u-mona',
    username: 'mona',
    displayName: 'Mona Hassan',
    role: 'member',
    status: 'pending',
    created: DateTime.utc(2026, 9, 26, 18, 20),
    initials: '',
    isSelf: false,
    createdLabel: '',
    passwordChangeRequired: false,
  );

  /// Admin → Users.
  static final AdminUsersView adminUsersView = AdminUsersView(
    availability: Availability.available,
    pending: [adminUserItem],
    users: [
      AdminUserItem(
        id: 'u-shawket',
        username: 'shawket',
        displayName: 'Shawket',
        role: 'admin',
        status: 'active',
        created: DateTime.utc(2026, 1, 4, 9),
        initials: '',
        isSelf: false,
        createdLabel: '',
        passwordChangeRequired: false,
      ),
    ],
    query: '',
  );

  /// "Petrol Arrows invoice · due today 10:00".
  static final NotificationOp notificationOp = NotificationOp(
    kind: NotificationOpKind.schedule,
    id: 4211,
    at: DateTime.utc(2026, 9, 27, 7),
    title: 'Petrol Arrows invoice',
    body: 'due today 10:00',
    taskId: 't-petrol-arrows',
  );

  /// Snooze (for the length set in Settings).
  static const NotificationAction notificationAction = NotificationAction(
    kind: NotificationActionKind.snooze,
  );

  // ---------------------------------------------------------------------------
  // Further view-model types (defaults; build variants with the constructors)
  // ---------------------------------------------------------------------------

  /// A sample [AiActivityItem].
  static const AiActivityItem aiActivityItem = AiActivityItem(
    atLabel: '',
    kind: '',
    summary: '',
    decisionId: '',
    reverted: false,
  );

  /// A sample [AiStatusView].
  static const AiStatusView aiStatusView = AiStatusView(
    enabled: false,
    queueDepth: 0,
    budgetUsedPercent: 0,
    budgetLabel: '',
  );

  /// A sample [AnnotatedLine].
  static const AnnotatedLine annotatedLine = AnnotatedLine(
    line: 0,
    text: '',
    change: LineChange.same,
    dir: TextDir.ltr,
  );

  /// A sample [AskScope].
  static const AskScope askScope = AskScope(kind: AskScopeKind.all, label: '');

  /// A sample [AskSource].
  static final AskSource askSource = AskSource(
    noteId: '',
    title: '',
    path: '',
    anchors: [],
    indexes: Uint32List(0),
  );

  /// A sample [AskSpan].
  static const AskSpan askSpan = AskSpan(text: '');

  /// A sample [BlockItem].
  static const BlockItem blockItem = BlockItem(
    text: '',
    textDir: TextDir.ltr,
    line: 0,
  );

  /// A sample [CitationPreview].
  static const CitationPreview citationPreview = CitationPreview(
    title: '',
    path: '',
    blockDir: TextDir.ltr,
    tags: [],
  );

  /// A sample [CompletionItem].
  static const CompletionItem completionItem = CompletionItem(
    label: '',
    detail: '',
    insertText: '',
    labelDir: TextDir.ltr,
  );

  /// A sample [Completions].
  static const Completions completions = Completions(
    kind: CompletionKind.none,
    replaceStart: 0,
    replaceEnd: 0,
    query: '',
    items: [],
  );

  /// A sample [CustodyDraft].
  static final CustodyDraft custodyDraft = CustodyDraft(kind: '', date: now);

  /// A sample [DeviceItem].
  static final DeviceItem deviceItem = DeviceItem(
    id: '',
    name: '',
    platform: '',
    lastSeen: now,
    lastSeenLabel: '',
    signedIn: now,
    signedInLabel: '',
    isThisDevice: false,
    remindersEnabled: false,
  );

  /// A sample [DiffLine].
  static const DiffLine diffLine = DiffLine(
    kind: DiffLineKind.same,
    text: '',
    dir: TextDir.ltr,
  );

  /// A sample [DirectoryFilter].
  static const DirectoryFilter directoryFilter = DirectoryFilter(
    tags: [],
    expiring: false,
    hasOpenItems: false,
  );

  /// A sample [DirectorySection].
  static const DirectorySection directorySection = DirectorySection(
    label: '',
    items: [],
  );

  /// A sample [DocumentDraft].
  static const DocumentDraft documentDraft = DocumentDraft(
    name: '',
    aliases: [],
    companies: [],
    people: [],
  );

  /// A sample [ExportSummary].
  static final ExportSummary exportSummary = ExportSummary(
    path: '',
    sizeBytes: BigInt.zero,
    noteCount: 0,
    label: '',
  );

  /// A sample [FilterOption].
  static const FilterOption filterOption = FilterOption(
    facet: '',
    value: '',
    label: '',
    count: 0,
    selected: false,
  );

  /// A sample [GraphFilter].
  static const GraphFilter graphFilter = GraphFilter(
    edgeKinds: [],
    nodeKinds: [],
    similarity: false,
    lens: GraphLens.notes,
    includeTags: false,
  );

  /// A sample [GraphPoint].
  static const GraphPoint graphPoint = GraphPoint(x: 0, y: 0);

  /// A sample [HistoryEntry].
  static final HistoryEntry historyEntry = HistoryEntry(
    commit: '',
    versionLabel: '',
    message: '',
    author: '',
    at: now,
    atLabel: '',
    canRevert: false,
  );

  /// A sample [ImportSummary].
  static const ImportSummary importSummary = ImportSummary(
    imported: 0,
    skipped: 0,
  );

  /// A sample [InboxPreviewItem].
  static const InboxPreviewItem inboxPreviewItem = InboxPreviewItem(
    noteId: '',
    text: '',
    textDir: TextDir.ltr,
    summary: '',
    needsYou: false,
  );

  /// A sample [IntegrityItem].
  static const IntegrityItem integrityItem = IntegrityItem(
    id: '',
    kind: '',
    messageKey: '',
    createdLabel: '',
  );

  /// A sample [KindCount].
  static const KindCount kindCount = KindCount(kind: '', label: '', count: 0);

  /// A sample [LinkOrCreateChoice].
  static const LinkOrCreateChoice linkOrCreateChoice = LinkOrCreateChoice(
    kind: LinkOrCreateKind.link,
    force: false,
  );

  /// A sample [MentionEdit].
  static const MentionEdit mentionEdit = MentionEdit(content: '', cursor: 0);

  /// A sample [MergePreview].
  static const MergePreview mergePreview = MergePreview(
    source: ahmedSamirRef,
    into: ahmedSamirRef,
    aliases: [],
    mentionCount: 0,
    relationCount: 0,
  );

  /// A sample [NavView].
  static final NavView navView = NavView(
    inboxCount: 0,
    tasksDueCount: 0,
    notesCount: 0,
    directoryCount: 0,
    clusterCount: 0,
    pinned: [],
    sync_: syncPill,
  );

  /// A sample [NewUserRequest].
  static const NewUserRequest newUserRequest = NewUserRequest(
    username: '',
    displayName: '',
    password: '',
    role: '',
  );

  /// A sample [NodePosition].
  static const NodePosition nodePosition = NodePosition(id: '', x: 0, y: 0);

  /// A sample [NoteDiffView].
  static const NoteDiffView noteDiffView = NoteDiffView(
    noteId: '',
    commit: '',
    summary: '',
    lines: [],
  );

  /// A sample [OpenItem].
  static const OpenItem openItem = OpenItem(
    id: '',
    text: '',
    textDir: TextDir.ltr,
    person: ahmedSamirRef,
    done: false,
  );

  /// A sample [PasswordStrength].
  static const PasswordStrength passwordStrength = PasswordStrength(
    level: PasswordLevel.tooShort,
    length: 0,
    minLength: 0,
  );

  /// A sample [PendingApproval].
  static final PendingApproval pendingApproval = PendingApproval(
    username: '',
    serverUrl: '',
    requestedAt: now,
    requestedLabel: '',
    canCheck: false,
  );

  /// A sample [PlaceDraft].
  static const PlaceDraft placeDraft = PlaceDraft(name: '', aliases: []);

  /// A sample [PlaceNode].
  static const PlaceNode placeNode = PlaceNode(
    place: ahmedSamirRef,
    depth: 0,
    documentCount: 0,
    parentId: '',
  );

  /// A sample [PlaceOption].
  static const PlaceOption placeOption = PlaceOption(
    id: '',
    title: '',
    breadcrumb: [],
    depth: 0,
    isCurrent: false,
  );

  /// A sample [RecentNotesView].
  static const RecentNotesView recentNotesView = RecentNotesView(
    filter: RecentFilter.edited,
    notes: [],
  );

  /// A sample [RecurrenceCompose].
  static const RecurrenceCompose recurrenceCompose = RecurrenceCompose(
    phrase: '',
    understood: false,
    label: '',
  );

  /// A sample [RecurrenceForm].
  static final RecurrenceForm recurrenceForm = RecurrenceForm(
    frequency: RecurrenceFrequency.daily,
    interval: 0,
    weekdays: [],
    monthDayMode: MonthDayMode.sameDay,
    monthDays: Uint32List(0),
    nth: 0,
    months: Uint32List(0),
    whenDone: false,
  );

  /// A sample [RecurrencePreviewItem].
  static final RecurrencePreviewItem recurrencePreviewItem =
      RecurrencePreviewItem(date: now, label: '', isDue: false);

  /// A sample [RelationTypeItem].
  static const RelationTypeItem relationTypeItem = RelationTypeItem(
    key: '',
    label: '',
  );

  /// A sample [SuggestionEdits].
  static const SuggestionEdits suggestionEdits = SuggestionEdits();

  /// A sample [SyncLogItem].
  static final SyncLogItem syncLogItem = SyncLogItem(
    at: now,
    atLabel: '',
    kind: '',
    detail: '',
  );

  /// A sample [TagItem].
  static const TagItem tagItem = TagItem(tag: '', count: 0);

  /// A sample [TaskChip].
  static const TaskChip taskChip = TaskChip(kind: TaskChipKind.due, label: '');

  /// A sample [TaskDraftPreview].
  static final TaskDraftPreview taskDraftPreview = TaskDraftPreview(
    description: '',
    descriptionDir: TextDir.ltr,
    reminders: [],
    links: [],
    chips: [],
    draft: taskDraft,
  );

  /// A sample [TaskGroup].
  static const TaskGroup taskGroup = TaskGroup(label: '', tasks: []);

  /// A sample [TaskHomeItem].
  static const TaskHomeItem taskHomeItem = TaskHomeItem(
    title: '',
    path: '',
    isDefault: false,
    openTasks: 0,
  );

  /// A sample [TaskHomesView].
  static const TaskHomesView taskHomesView = TaskHomesView(homes: []);

  /// A sample [TextSpan].
  static const TextSpan textSpan = TextSpan(start: 0, end: 0);

  /// A sample [ThreadMessage].
  static const ThreadMessage threadMessage = ThreadMessage(
    id: '',
    author: '',
    text: '',
    textDir: TextDir.ltr,
    createdLabel: '',
    pendingSync: false,
  );

  /// A sample [TimelineChip].
  static const TimelineChip timelineChip = TimelineChip(
    dateLabel: '',
    sourcePhrase: '',
    targets: [],
  );
}
