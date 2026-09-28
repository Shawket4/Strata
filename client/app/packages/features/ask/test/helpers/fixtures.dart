import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Ask and search fixtures for this package's tests.
abstract final class AskFixtures {
  /// A mixed-script conversation, Ask available.
  static const AskView available = AskView(
    availability: Availability.available,
    messages: [
      AskMessage(
        role: 'user',
        text: 'What did Acme ask for on invoicing, and did we agree?',
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
      AskMessage(
        role: 'assistant',
        text:
            'Acme asked to move from monthly to weekly invoicing, starting in '
            'October. No agreement is recorded yet.',
        citations: [
          StrataFixtures.citation,
          Citation(
            noteId: 'n-weekly-invoicing',
            target: 'Weekly invoicing proposal',
            anchor: 'd2e5',
          ),
        ],
        id: '',
        streaming: false,
        spans: [],
        sources: [],
        scopeLabel: '',
        sourceCount: 0,
        createdLabel: '',
        dir: TextDir.ltr,
      ),
      AskMessage(
        role: 'user',
        text: 'ومنى قالت إيه عن أسعار Nile Freight؟',
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
      AskMessage(
        role: 'assistant',
        text: 'منى حسن قالت إن أسعار Nile Freight هتزيد ٨٪ ابتداءً من نوفمبر.',
        citations: [
          Citation(target: 'Nile Freight rate increase', anchor: 'r8p2'),
        ],
        id: '',
        streaming: false,
        spans: [],
        sources: [],
        scopeLabel: '',
        sourceCount: 0,
        createdLabel: '',
        dir: TextDir.ltr,
      ),
    ],
    scopes: [],
    streaming: false,
  );

  /// Offline: the conversation stays readable.
  static const AskView offline = AskView(
    availability: Availability.offline,
    messages: [],
    scopes: [],
    streaming: false,
  );

  /// Nothing asked yet.
  static const AskView empty = AskView(
    availability: Availability.available,
    messages: [],
    scopes: [],
    streaming: false,
  );

  /// Semantic search while offline.
  static const SearchView semanticOffline = SearchView(
    query: 'pricing',
    mode: SearchMode.semantic,
    results: [],
    availability: Availability.offline,
    availableModes: [],
  );

  /// No results.
  static const SearchView noResults = SearchView(
    query: 'zz',
    mode: SearchMode.keyword,
    results: [],
    availability: Availability.available,
    availableModes: [],
  );
}
