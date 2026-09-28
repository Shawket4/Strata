import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// Directory fixtures for this package's tests.
abstract final class DirFixtures {
  /// The Documents tab.
  static const DirectoryView documents = DirectoryView(
    tab: DirectoryTab.documents,
    query: '',
    items: [
      DirectoryItem(
        id: 'd-watanya-contract',
        title: 'Watanya contract',
        subtitle: 'Stored · Nasr City office › Safe',
        aliases: ['عقد وطنية'],
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
      DirectoryItem(
        id: 'd-car-licence',
        title: 'Car licence',
        subtitle: 'Checked out · with Shawket',
        aliases: ['رخصة العربية'],
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
    counts: StrataFixtures.directoryCounts,
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// The Companies tab.
  static const DirectoryView companies = DirectoryView(
    tab: DirectoryTab.companies,
    query: '',
    items: [
      DirectoryItem(
        id: 'c-acme-logistics',
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
      ),
    ],
    counts: StrataFixtures.directoryCounts,
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// A search in both scripts ("أحمد").
  static const DirectoryView peopleSearch = DirectoryView(
    tab: DirectoryTab.people,
    query: 'أحمد',
    items: [StrataFixtures.directoryItem],
    counts: StrataFixtures.directoryCounts,
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// A search without results.
  static const DirectoryView noMatches = DirectoryView(
    tab: DirectoryTab.people,
    query: 'zz',
    items: [],
    counts: StrataFixtures.directoryCounts,
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// An empty directory.
  static const DirectoryView emptyPeople = DirectoryView(
    tab: DirectoryTab.people,
    query: '',
    items: [],
    counts: DirectoryCounts(people: 0, companies: 0, documents: 0, places: 0),
    filter: DirectoryFilter(tags: [], expiring: false, hasOpenItems: false),
    sort: DirectorySort.name,
    filterOptions: [],
    sections: [],
    suggestions: [],
    expiringCount: 0,
  );

  /// "Who is “أبو علي”?" with one candidate.
  static final SuggestionItem whoIs = SuggestionItem(
    id: 's-who-is-abu-ali',
    noteId: 'n-capture-abu-ali',
    status: 'pending',
    detail: const SuggestionDetail(
      kind: SuggestionKind.entityLink,
      title: '',
      folder: '',
      tags: [],
      mention: 'أبو علي',
      candidates: [StrataFixtures.ahmedSamirRef],
      line: '',
      confidence: 0.72,
      duplicates: [],
      relType: '',
      reason: 'Called “Abu Ali” in two notes about Acme.',
      serverKind: 'entity_link',
      documentChoices: [],
      entityKind: 'person',
      isNickname: true,
      quote: '',
      entities: [],
    ),
    created: DateTime.utc(2026, 9, 27, 8),
    pendingSync: false,
    createdLabel: '2 hours ago',
    sourceText: 'أبو علي هيبعت العقد بكرة',
    sourceDir: TextDir.rtl,
    autoApplied: false,
    canAccept: false,
    needsYou: true,
    thread: [],
  );

  /// The People tab with the core's sections and the who-is suggestion.
  static final DirectoryView people = DirectoryView(
    tab: DirectoryTab.people,
    query: '',
    items: StrataFixtures.directoryView.items,
    counts: StrataFixtures.directoryCounts,
    filter: StrataFixtures.directoryView.filter,
    sort: DirectorySort.name,
    filterOptions: StrataFixtures.directoryView.filterOptions,
    sections: [
      DirectorySection(
        label: 'All people · A–Z',
        items: StrataFixtures.directoryView.items,
      ),
    ],
    suggestions: [whoIs],
    expiringCount: 0,
  );

  /// The People tab filtered to the role "Operations manager".
  static const DirectoryView peopleByRole = DirectoryView(
    tab: DirectoryTab.people,
    query: '',
    items: [StrataFixtures.directoryItem],
    counts: StrataFixtures.directoryCounts,
    filter: DirectoryFilter(
      tags: [],
      role: 'Operations manager',
      expiring: false,
      hasOpenItems: false,
    ),
    sort: DirectorySort.name,
    filterOptions: [
      FilterOption(
        facet: 'role',
        value: 'Operations manager',
        label: 'Operations manager',
        count: 1,
        selected: true,
      ),
    ],
    sections: [
      DirectorySection(label: 'Results', items: [StrataFixtures.directoryItem]),
    ],
    suggestions: [],
    expiringCount: 0,
  );

  /// Ahmed Samir with unsynced changes (offline edits).
  static const EntityScreen ahmedPending = EntityScreen(
    id: 'p-ahmed-samir',
    kind: EntityPageKind.entity,
    entity: EntityView(
      id: 'p-ahmed-samir',
      kind: 'person',
      title: 'Ahmed Samir',
      aliases: [],
      properties: [],
      insights: [],
      openItems: [],
      timeline: [],
      mentions: [],
      related: [],
      documents: [StrataFixtures.documentBrief],
      pendingSync: true,
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
    ),
  );

  /// An unknown ID.
  static const EntityScreen notFound = EntityScreen(
    id: 'x-gone',
    kind: EntityPageKind.notFound,
  );
}
