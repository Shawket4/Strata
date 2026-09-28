import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

SuggestionDetail _detail({
  required SuggestionKind kind,
  String title = '',
  String folder = '',
  List<String> tags = const [],
  String mention = '',
  List<EntityRef> candidates = const [],
  EntityRef? document,
  String line = '',
  double? confidence,
  List<CandidateItem> duplicates = const [],
  EntityRef? target,
  String relType = '',
  String reason = '',
  String serverKind = '',
}) => SuggestionDetail(
  kind: kind,
  title: title,
  folder: folder,
  tags: tags,
  mention: mention,
  candidates: candidates,
  document: document,
  line: line,
  confidence: confidence,
  duplicates: duplicates,
  target: target,
  relType: relType,
  reason: reason,
  serverKind: serverKind,
  documentChoices: [],
  entityKind: '',
  isNickname: false,
  quote: '',
  entities: [],
);

SuggestionItem _suggestion(
  String id,
  SuggestionDetail detail, {
  String? noteId,
  String status = 'pending',
  bool pendingSync = false,
}) => SuggestionItem(
  id: id,
  noteId: noteId,
  status: status,
  detail: detail,
  created: DateTime.utc(2026, 9, 27, 8),
  pendingSync: pendingSync,
  createdLabel: '',
  sourceDir: TextDir.ltr,
  autoApplied: false,
  canAccept: false,
  needsYou: false,
  thread: [],
);

/// Inbox fixtures beyond `StrataFixtures` (SCREEN_SPEC sample content).
abstract final class InboxFixtures {
  static final SuggestionItem loyaltyFiling = _suggestion(
    's-filing-loyalty',
    _detail(
      kind: SuggestionKind.filing,
      title: 'Loyalty tier for 12-month customers',
      folder: 'notes/product',
      tags: const ['loyalty', 'pricing'],
      confidence: 0.84,
    ),
    noteId: 'n-capture-loyalty',
  );

  static final SuggestionItem contradicts = _suggestion(
    's-rel-contradicts',
    _detail(
      kind: SuggestionKind.correction,
      target: const EntityRef(
        id: 'n-discount-policy',
        title: 'Discount policy',
      ),
      relType: 'contradicts',
      confidence: 0.91,
      reason: 'Discount policy caps discounts at 3%; this proposes 5%.',
    ),
    noteId: 'n-capture-loyalty',
  );

  static final SuggestionItem partOf = _suggestion(
    's-rel-part-of',
    _detail(
      kind: SuggestionKind.correction,
      target: const EntityRef(
        id: 'n-subscription-tiers',
        title: 'Subscription tiers',
      ),
      relType: 'part-of',
      confidence: 0.84,
    ),
    noteId: 'n-capture-loyalty',
  );

  static final InboxItem loyaltyCapture = InboxItem(
    noteId: 'n-capture-loyalty',
    title: '2026-09-27 10-12',
    text: 'Idea: loyalty tier for customers > 12 months, 5% off renewals',
    created: DateTime.utc(2026, 9, 27, 7, 12),
    suggestions: [loyaltyFiling, contradicts, partOf],
    pendingSync: false,
    textDir: TextDir.ltr,
    createdLabel: '',
    needsYou: false,
    ready: false,
    isDuplicate: false,
  );

  static final SuggestionItem custodyApplied = _suggestion(
    's-custody-applied',
    _detail(
      kind: SuggestionKind.custody,
      document: StrataFixtures.watanyaContractRef,
      line: 'Returned to Safe — Nasr City office · last holder Shady',
      confidence: 0.93,
    ),
    status: 'accepted',
  );

  static final SuggestionItem custodyAmbiguous = _suggestion(
    's-custody-which',
    _detail(
      kind: SuggestionKind.custody,
      line: 'Handed to Shady · today',
      candidates: const [
        StrataFixtures.watanyaContractRef,
        EntityRef(
          id: 'd-petrol-arrows-register',
          title: 'Petrol Arrows commercial register',
        ),
      ],
      confidence: 0.62,
    ),
  );

  static final SuggestionItem duplicate = _suggestion(
    's-duplicate-watanya',
    _detail(
      kind: SuggestionKind.duplicate,
      duplicates: const [StrataFixtures.candidateItem],
    ),
    noteId: 'n-capture-watanya',
  );

  static final SuggestionItem task = _suggestion(
    's-task-watanya',
    _detail(
      kind: SuggestionKind.task,
      line: "Make Watanya's ETA invoice · every month on the 1st",
      confidence: 0.8,
    ),
  );

  static final SuggestionItem unsupported = _suggestion(
    's-merge',
    _detail(kind: SuggestionKind.unsupported, serverKind: 'entity-merge'),
  );

  /// The full inbox: captures (mixed scripts) and standalone suggestions.
  static final InboxView full = InboxView(
    captures: [
      StrataFixtures.inboxItem,
      loyaltyCapture,
      StrataFixtures.inboxItemEnglish,
    ],
    suggestions: [StrataFixtures.suggestionLinkOrCreate],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );

  /// Custody items (CustodyInboxCompact).
  static final InboxView custody = InboxView(
    captures: const [],
    suggestions: [custodyApplied, custodyAmbiguous],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );

  /// Link-or-create only.
  static final InboxView linkOrCreate = InboxView(
    captures: const [],
    suggestions: [StrataFixtures.suggestionLinkOrCreate],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );

  /// Duplicate-flagged, task and unsupported suggestions.
  static final InboxView others = InboxView(
    captures: const [],
    suggestions: [duplicate, task, unsupported],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );

  static const InboxView empty = InboxView(
    captures: [],
    suggestions: [],
    filter: InboxFilter.all,
    readyCount: 0,
    needsYouCount: 0,
    conflictsCount: 0,
    allCount: 0,
  );
}
