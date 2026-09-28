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
  String get sortBy => 'Sort';

  @override
  String get sortName => 'Name A–Z';

  @override
  String get sortLastActive => 'Last active';

  @override
  String get sortRecentlyMoved => 'Recently moved';

  @override
  String filterOption({required String label, required int count}) {
    return '$label · $count';
  }

  @override
  String rowActivity({required int count, required String when}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count mentions',
      one: '1 mention',
      zero: 'No mentions',
    );
    return '$_temp0 · $when';
  }

  @override
  String get colActivity => 'Activity';

  @override
  String mentionCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count mentions',
      one: '1 mention',
      zero: 'No mentions',
    );
    return '$_temp0';
  }

  @override
  String get pickerSearch => 'Search';

  @override
  String repointTitle({required String title}) {
    return 'Point $title to…';
  }

  @override
  String actionFailed({required String code}) {
    return 'That didn\'t work ($code).';
  }

  @override
  String mergeInto({required String title}) {
    return 'Merge $title into…';
  }

  @override
  String get mergeTitle => 'Merge pages';

  @override
  String mergeBody({required String source, required String into}) {
    return '$source will be merged into $into.';
  }

  @override
  String mergeMoves({required int mentions, required int relations}) {
    String _temp0 = intl.Intl.pluralLogic(
      mentions,
      locale: localeName,
      other: '$mentions mentions',
      one: '1 mention',
      zero: 'no mentions',
    );
    String _temp1 = intl.Intl.pluralLogic(
      relations,
      locale: localeName,
      other: '$relations relations',
      one: '1 relation',
      zero: 'no relations',
    );
    return 'Moves $_temp0 and $_temp1.';
  }

  @override
  String get mergeAliases => 'Aliases added';

  @override
  String get mergeConfirm => 'Merge';

  @override
  String get merged => 'Merged';

  @override
  String get addAlias => 'Add alias';

  @override
  String get aliasField => 'Alias';

  @override
  String get add => 'Add';

  @override
  String get addProperty => 'Add property';

  @override
  String editProperty({required String key}) {
    return 'Edit $key';
  }

  @override
  String get propertyKey => 'Property';

  @override
  String get propertyValue => 'Value';

  @override
  String get save => 'Save';

  @override
  String propertyActions({required String key}) {
    return '$key actions';
  }

  @override
  String get edit => 'Edit';

  @override
  String get remove => 'Remove';

  @override
  String removeAlias({required String alias}) {
    return 'Remove alias $alias';
  }

  @override
  String entitySubtitleActive({
    required String subtitle,
    required String when,
  }) {
    return '$subtitle · active $when';
  }

  @override
  String aiUpdated({required String when}) {
    return 'AI-maintained · updated $when';
  }

  @override
  String openDone({required int open, required int done}) {
    return '$open open · $done done';
  }

  @override
  String createExists({required String title}) {
    return 'Already exists: $title';
  }

  @override
  String linkTo({required String title}) {
    return 'It\'s $title';
  }

  @override
  String get createCompany => 'Create company…';
}
