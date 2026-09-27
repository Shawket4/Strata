// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'directory_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class DirectoryLocalizationsEn extends DirectoryLocalizations {
  DirectoryLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get directoryTitle => 'Directory';

  @override
  String get tabsLabel => 'Directory sections';

  @override
  String get tabPeople => 'People';

  @override
  String get tabCompanies => 'Companies';

  @override
  String get tabDocuments => 'Documents';

  @override
  String get tabPlaces => 'Places';

  @override
  String tabWithCount({required String tab, required int count}) {
    return '$tab · $count';
  }

  @override
  String get searchPeople => 'Search people';

  @override
  String get searchCompanies => 'Search companies';

  @override
  String get searchDocuments => 'Find a document or ask where it is';

  @override
  String get searchPlaces => 'Search places';

  @override
  String get searchHint => 'Search in English or العربية';

  @override
  String get searchHintDocuments => 'Where is…? / فين…؟';

  @override
  String get filtersLabel => 'Filters';

  @override
  String get filterTag => 'Tag';

  @override
  String get filterRole => 'Role';

  @override
  String get filterCompany => 'Company';

  @override
  String get filterIndustry => 'Industry';

  @override
  String get filterType => 'Type';

  @override
  String get filterStatus => 'Status';

  @override
  String get filterPlace => 'Place';

  @override
  String get filterExpiring => 'Expiring';

  @override
  String get filterHolder => 'Holder';

  @override
  String get filtersUnavailable => 'Filters aren\'t available yet';

  @override
  String peopleCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count people',
      one: '1 person',
      zero: 'No people',
    );
    return '$_temp0';
  }

  @override
  String companiesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count companies',
      one: '1 company',
      zero: 'No companies',
    );
    return '$_temp0';
  }

  @override
  String documentsCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count documents',
      one: '1 document',
      zero: 'No documents',
    );
    return '$_temp0';
  }

  @override
  String placesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count places',
      one: '1 place',
      zero: 'No places',
    );
    return '$_temp0';
  }

  @override
  String get emptyPeople => 'No people yet';

  @override
  String get emptyCompanies => 'No companies yet';

  @override
  String get emptyDocuments => 'No documents yet';

  @override
  String get emptyPlaces => 'No places yet';

  @override
  String get emptyMessage => 'Mentions in your notes and captures create them.';

  @override
  String noMatches({required String query}) {
    return 'No matches for “$query”';
  }

  @override
  String get noMatchesMessage =>
      'Search looks at names and aliases in both scripts.';

  @override
  String get suggestions => 'Suggestions';

  @override
  String get suggestionsHint => 'Nothing changes until you accept';

  @override
  String whoIs({required String mention}) {
    return 'Who is “$mention”?';
  }

  @override
  String whoIsCandidates({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count possible matches',
      one: '1 possible match',
      zero: 'No matching person',
    );
    return '$_temp0';
  }

  @override
  String get possibleDuplicate => 'Possible duplicate';

  @override
  String get custodySuggestion => 'Custody update to confirm';

  @override
  String get relationSuggestion => 'Relation to review';

  @override
  String get taskSuggestion => 'Task from a capture';

  @override
  String get filingSuggestion => 'Filing to review';

  @override
  String get otherSuggestion => 'Suggestion';

  @override
  String get accept => 'Accept';

  @override
  String get dismiss => 'Dismiss';

  @override
  String get createPerson => 'Create person…';

  @override
  String aiConfidence({required String value}) {
    return 'AI · $value';
  }

  @override
  String get newPerson => 'New person';

  @override
  String get newCompany => 'New company';

  @override
  String get nameField => 'Name';

  @override
  String get aliasesField => 'Aliases (one per line)';

  @override
  String get create => 'Create';

  @override
  String get createAnyway => 'Create anyway';

  @override
  String get cancel => 'Cancel';

  @override
  String get alreadyExists => 'Already exists';

  @override
  String matchScore({required String level, required String score}) {
    return '$level · $score';
  }

  @override
  String get openExisting => 'Open existing';

  @override
  String get colName => 'Name';

  @override
  String get colAliases => 'Aliases';

  @override
  String get colDetails => 'Details';

  @override
  String tableCaption({required String tab}) {
    return '$tab, by name';
  }

  @override
  String get keyboardHint => 'move · open page';

  @override
  String get preview => 'Preview';

  @override
  String previewOf({required String title}) {
    return 'Preview: $title';
  }

  @override
  String get closePreview => 'Close preview';

  @override
  String get openPage => 'Open page';

  @override
  String get merge => 'Merge…';

  @override
  String get mergeUnavailable => 'Merging isn\'t available yet';

  @override
  String get refreshInsights => 'Refresh insights';

  @override
  String get refreshQueued => 'Insights refresh queued';

  @override
  String get summary => 'Summary';

  @override
  String get aiMaintained => 'AI-maintained';

  @override
  String get insights => 'Insights';

  @override
  String get openItems => 'Open items';

  @override
  String get timeline => 'Timeline';

  @override
  String get noSummary => 'No summary yet.';

  @override
  String get nothingYet => 'Nothing yet.';

  @override
  String get relatedEntities => 'Related entities';

  @override
  String get noRelated => 'No related people or companies yet.';

  @override
  String rejectRelation({required String type, required String title}) {
    return 'Reject relation $type $title';
  }

  @override
  String repointRelation({required String title}) {
    return 'Repoint $title';
  }

  @override
  String get repointUnavailable => 'Repointing isn\'t available yet';

  @override
  String get mentioningNotes => 'Mentioning notes';

  @override
  String get newestFirst => 'Newest first';

  @override
  String get noMentions => 'No notes mention this yet.';

  @override
  String get documentsSection => 'Documents';

  @override
  String get entityGraph => 'Entity graph';

  @override
  String get yourNotes => 'Your notes';

  @override
  String get yourNotesHint => 'Only you edit this section · ## Notes';

  @override
  String get yourNotesUnavailable =>
      'Editing your notes here isn\'t available yet. Open the note to edit its ## Notes section.';

  @override
  String get tabOverview => 'Overview';

  @override
  String tabNotes({required int count}) {
    return 'Notes · $count';
  }

  @override
  String get tabGraph => 'Graph';

  @override
  String get entityViews => 'Entity views';

  @override
  String get kindPerson => 'Person';

  @override
  String get kindCompany => 'Company';

  @override
  String entitySubtitle({required String kind, required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count mentioning notes',
      one: '1 mentioning note',
      zero: 'no mentioning notes',
    );
    return '$kind · $_temp0';
  }

  @override
  String get backToPeople => 'Back to People';

  @override
  String get backToCompanies => 'Back to Companies';

  @override
  String get openNote => 'Open markdown file';

  @override
  String get pendingSync => 'Not synced yet';

  @override
  String get loading => 'Loading…';

  @override
  String get errorTitle => 'This couldn\'t be loaded';

  @override
  String errorMessage({required String code}) {
    return 'The app\'s local data returned an error ($code).';
  }

  @override
  String get notFoundTitle => 'This page isn\'t in the vault any more';

  @override
  String get notFoundMessage =>
      'It may have been deleted or merged on another device.';

  @override
  String get selectSomething => 'Select an item to see it here';

  @override
  String dateShort({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String mentionDate({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }
}
