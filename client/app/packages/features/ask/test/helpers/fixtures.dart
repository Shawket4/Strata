import 'dart:typed_data';

import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Ask and search fixtures for this package's tests.
abstract final class AskFixtures {
  /// The scopes the core offers.
  static const List<AskScope> scopes = [
    AskScope(kind: AskScopeKind.all, label: 'All notes'),
    AskScope(
      kind: AskScopeKind.entity,
      value: 'c-acme-logistics',
      label: 'Acme Logistics',
    ),
    AskScope(
      kind: AskScopeKind.folder,
      value: 'notes/clients',
      label: 'notes/clients',
    ),
  ];

  /// AI on, 62 % of today's budget used.
  static const AiStatusView aiStatus = AiStatusView(
    enabled: true,
    provider: 'OpenAI',
    queueDepth: 0,
    failedJobs: 0,
    budgetUsedPercent: 62,
    budgetLabel: '62% used',
  );

  /// The question about Acme.
  static const AskMessage question = AskMessage(
    role: 'user',
    text: 'What did Acme ask for on invoicing, and did we agree?',
    citations: [],
    id: 'q-1',
    streaming: false,
    spans: [],
    sources: [],
    scopeLabel: '',
    sourceCount: 0,
    createdLabel: '14:04',
    dir: TextDir.ltr,
  );

  /// The answer with two inline citations.
  static final AskMessage answer = AskMessage(
    role: 'assistant',
    text:
        'Acme asked to move from monthly to weekly invoicing, starting in '
        'October. No agreement is recorded yet.',
    citations: const [
      StrataFixtures.citation,
      Citation(
        noteId: 'n-weekly-invoicing',
        target: 'Weekly invoicing proposal',
        anchor: 'd2e5',
      ),
    ],
    id: 'a-1',
    streaming: false,
    spans: const [
      AskSpan(
        text:
            'Acme asked to move from monthly to weekly invoicing, starting '
            'in October. ',
      ),
      AskSpan(text: '', citation: 1),
      AskSpan(text: ' No agreement is recorded yet. '),
      AskSpan(text: '', citation: 2),
    ],
    sources: [
      AskSource(
        noteId: 'n-call-2026-09-12-acme',
        title: 'Call 2026-09-12 — Acme',
        path: 'notes/clients/acme/Call 2026-09-12 — Acme.md',
        anchors: const ['a1b2'],
        indexes: Uint32List.fromList([1]),
      ),
      AskSource(
        noteId: 'n-weekly-invoicing',
        title: 'Weekly invoicing proposal',
        path: 'notes/clients/acme/Weekly invoicing proposal.md',
        anchors: const ['d2e5'],
        indexes: Uint32List.fromList([2]),
      ),
    ],
    scopeLabel: 'Scope: All notes',
    sourceCount: 2,
    createdLabel: '14:05',
    dir: TextDir.ltr,
  );

  /// The Arabic follow-up and its answer.
  static const AskMessage questionArabic = AskMessage(
    role: 'user',
    text: 'ومنى قالت إيه عن أسعار Nile Freight؟',
    citations: [],
    id: 'q-2',
    streaming: false,
    spans: [],
    sources: [],
    scopeLabel: '',
    sourceCount: 0,
    createdLabel: '14:06',
    dir: TextDir.rtl,
  );

  /// An Arabic answer saved as a note.
  static const AskMessage answerArabic = AskMessage(
    role: 'assistant',
    text: 'منى حسن قالت إن أسعار Nile Freight هتزيد ٨٪ ابتداءً من نوفمبر.',
    citations: [Citation(target: 'Nile Freight rate increase', anchor: 'r8p2')],
    id: 'a-2',
    streaming: false,
    spans: [],
    sources: [],
    scopeLabel: 'Scope: All notes',
    sourceCount: 1,
    createdLabel: '14:06',
    dir: TextDir.rtl,
    savedNoteId: 'n-ask-nile-freight',
  );

  /// A mixed-script conversation, Ask available.
  static final AskView available = AskView(
    availability: Availability.available,
    messages: [question, answer, questionArabic, answerArabic],
    scopes: scopes,
    streaming: false,
    aiStatus: aiStatus,
  );

  /// A conversation about one note: its saved thread.
  static final AskView aboutNote = AskView(
    availability: Availability.available,
    note: const AskNoteScope(
      noteId: 'n-call-2026-09-12-acme',
      title: 'Call 2026-09-12 Acme',
      label: 'About Call 2026-09-12 Acme',
    ),
    messages: [question, answer],
    scopes: scopes,
    streaming: false,
    aiStatus: aiStatus,
  );

  /// An answer still streaming.
  static const AskView streaming = AskView(
    availability: Availability.available,
    messages: [
      question,
      AskMessage(
        role: 'assistant',
        text: 'Acme asked to move from monthly',
        citations: [],
        id: 'a-1',
        streaming: true,
        spans: [AskSpan(text: 'Acme asked to move from monthly')],
        sources: [],
        scopeLabel: 'Scope: All notes',
        sourceCount: 0,
        createdLabel: '14:05',
        dir: TextDir.ltr,
      ),
    ],
    scopes: scopes,
    streaming: true,
    aiStatus: aiStatus,
  );

  /// An answer cut short because AI was paused.
  static const AskView paused = AskView(
    availability: Availability.available,
    messages: [
      question,
      AskMessage(
        role: 'assistant',
        text: 'Acme asked to move from monthly',
        citations: [],
        id: 'a-1',
        streaming: false,
        spans: [],
        sources: [],
        scopeLabel: 'Scope: All notes',
        sourceCount: 0,
        createdLabel: '14:05',
        dir: TextDir.ltr,
        errorKey: 'error.ai_paused',
      ),
    ],
    scopes: scopes,
    streaming: false,
    aiStatus: AiStatusView(
      enabled: true,
      pausedLabel: 'Paused until 14:00',
      queueDepth: 3,
      failedJobs: 0,
      budgetUsedPercent: 100,
      budgetLabel: '100% used',
    ),
  );

  /// Offline: the conversation stays readable.
  static const AskView offline = AskView(
    availability: Availability.offline,
    messages: [],
    scopes: scopes,
    streaming: false,
  );

  /// Not available on this server.
  static const AskView notYet = AskView(
    availability: Availability.notYetAvailable,
    messages: [],
    scopes: [],
    streaming: false,
  );

  /// Nothing asked yet.
  static const AskView empty = AskView(
    availability: Availability.available,
    messages: [],
    scopes: scopes,
    streaming: false,
    aiStatus: aiStatus,
  );

  /// The block cited first.
  static const CitationPreview preview = CitationPreview(
    noteId: 'n-call-2026-09-12-acme',
    title: 'Call 2026-09-12 — Acme',
    path: 'notes/clients/acme/Call 2026-09-12 — Acme.md',
    blockText: 'Ahmed asked for weekly invoicing from October.',
    blockDir: TextDir.ltr,
    heading: 'Invoicing',
    dateLabel: '12 Sep 2026',
    tags: ['acme', 'invoicing'],
  );

  /// Hybrid search: highlighted matches and scores.
  static const SearchView hybrid = SearchView(
    query: 'pricing',
    mode: SearchMode.hybrid,
    results: [
      SearchHit(
        noteId: 'n-pricing-experiments',
        title: 'Pricing experiments',
        path: 'notes/sales/Pricing experiments.md',
        kind: 'note',
        snippet: '… pricing: a 5% loyalty discount on renewals …',
        titleDir: TextDir.ltr,
        snippetDir: TextDir.ltr,
        highlights: [HighlightSpan(start: 2, end: 9)],
        score: 0.91,
      ),
      SearchHit(
        noteId: 'n-pricing-summary-ar',
        title: 'تجارب التسعير — ملخص',
        path: 'notes/sales/تجارب التسعير — ملخص.md',
        kind: 'note',
        snippet: 'ملخص نتائج تجارب pricing للربع الثالث',
        titleDir: TextDir.rtl,
        snippetDir: TextDir.rtl,
        highlights: [HighlightSpan(start: 17, end: 24)],
        score: 0.74,
      ),
    ],
    availability: Availability.available,
    availableModes: [
      SearchMode.keyword,
      SearchMode.semantic,
      SearchMode.hybrid,
    ],
  );

  /// Keyword search with only the local mode available (offline).
  static const SearchView keywordOnly = SearchView(
    query: 'pricing',
    mode: SearchMode.keyword,
    results: [
      SearchHit(
        noteId: 'n-pricing-experiments',
        title: 'Pricing experiments',
        path: 'notes/sales/Pricing experiments.md',
        kind: 'note',
        snippet: '… pricing: a 5% loyalty discount on renewals …',
        titleDir: TextDir.ltr,
        snippetDir: TextDir.ltr,
        highlights: [HighlightSpan(start: 2, end: 9)],
        score: 0,
      ),
    ],
    availability: Availability.available,
    availableModes: [SearchMode.keyword],
  );

  /// Semantic search while offline.
  static const SearchView semanticOffline = SearchView(
    query: 'pricing',
    mode: SearchMode.semantic,
    results: [],
    availability: Availability.offline,
    availableModes: [SearchMode.keyword],
  );

  /// No results.
  static const SearchView noResults = SearchView(
    query: 'zz',
    mode: SearchMode.keyword,
    results: [],
    availability: Availability.available,
    availableModes: [SearchMode.keyword],
  );
}
