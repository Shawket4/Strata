import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'directory_localizations_ar.dart';
import 'directory_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of DirectoryLocalizations
/// returned by `DirectoryLocalizations.of(context)`.
///
/// Applications need to include `DirectoryLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/directory_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: DirectoryLocalizations.localizationsDelegates,
///   supportedLocales: DirectoryLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the DirectoryLocalizations.supportedLocales
/// property.
abstract class DirectoryLocalizations {
  DirectoryLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static DirectoryLocalizations of(BuildContext context) {
    return Localizations.of<DirectoryLocalizations>(
      context,
      DirectoryLocalizations,
    )!;
  }

  static const LocalizationsDelegate<DirectoryLocalizations> delegate =
      _DirectoryLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('en'),
  ];

  /// Directory title.
  ///
  /// In en, this message translates to:
  /// **'Directory'**
  String get directoryTitle;

  /// Tab bar label.
  ///
  /// In en, this message translates to:
  /// **'Directory sections'**
  String get tabsLabel;

  /// Tab.
  ///
  /// In en, this message translates to:
  /// **'People'**
  String get tabPeople;

  /// Tab.
  ///
  /// In en, this message translates to:
  /// **'Companies'**
  String get tabCompanies;

  /// Tab.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get tabDocuments;

  /// Tab.
  ///
  /// In en, this message translates to:
  /// **'Places'**
  String get tabPlaces;

  /// Search label.
  ///
  /// In en, this message translates to:
  /// **'Search people'**
  String get searchPeople;

  /// Search label.
  ///
  /// In en, this message translates to:
  /// **'Search companies'**
  String get searchCompanies;

  /// Search label.
  ///
  /// In en, this message translates to:
  /// **'Find a document or ask where it is'**
  String get searchDocuments;

  /// Search label.
  ///
  /// In en, this message translates to:
  /// **'Search places'**
  String get searchPlaces;

  /// Search placeholder.
  ///
  /// In en, this message translates to:
  /// **'Search in English or العربية'**
  String get searchHint;

  /// Documents search placeholder.
  ///
  /// In en, this message translates to:
  /// **'Where is…? / فين…؟'**
  String get searchHintDocuments;

  /// Filters group label.
  ///
  /// In en, this message translates to:
  /// **'Filters'**
  String get filtersLabel;

  /// Count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No people} =1{1 person} other{{count} people}}'**
  String peopleCount({required int count});

  /// Count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No companies} =1{1 company} other{{count} companies}}'**
  String companiesCount({required int count});

  /// Count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No documents} =1{1 document} other{{count} documents}}'**
  String documentsCount({required int count});

  /// Count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No places} =1{1 place} other{{count} places}}'**
  String placesCount({required int count});

  /// Empty tab.
  ///
  /// In en, this message translates to:
  /// **'No people yet'**
  String get emptyPeople;

  /// Empty tab.
  ///
  /// In en, this message translates to:
  /// **'No companies yet'**
  String get emptyCompanies;

  /// Empty tab.
  ///
  /// In en, this message translates to:
  /// **'No documents yet'**
  String get emptyDocuments;

  /// Empty tab.
  ///
  /// In en, this message translates to:
  /// **'No places yet'**
  String get emptyPlaces;

  /// Empty tab body.
  ///
  /// In en, this message translates to:
  /// **'Mentions in your notes and captures create them.'**
  String get emptyMessage;

  /// Search without results.
  ///
  /// In en, this message translates to:
  /// **'No matches for “{query}”'**
  String noMatches({required String query});

  /// Search without results body.
  ///
  /// In en, this message translates to:
  /// **'Search looks at names and aliases in both scripts.'**
  String get noMatchesMessage;

  /// Suggestion strip title.
  ///
  /// In en, this message translates to:
  /// **'Suggestions'**
  String get suggestions;

  /// Suggestion strip caption.
  ///
  /// In en, this message translates to:
  /// **'Nothing changes until you accept'**
  String get suggestionsHint;

  /// Link-or-create suggestion.
  ///
  /// In en, this message translates to:
  /// **'Who is “{mention}”?'**
  String whoIs({required String mention});

  /// Link-or-create detail.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No matching person} =1{1 possible match} other{{count} possible matches}}'**
  String whoIsCandidates({required int count});

  /// Duplicate suggestion.
  ///
  /// In en, this message translates to:
  /// **'Possible duplicate'**
  String get possibleDuplicate;

  /// Custody suggestion.
  ///
  /// In en, this message translates to:
  /// **'Custody update to confirm'**
  String get custodySuggestion;

  /// Relation suggestion.
  ///
  /// In en, this message translates to:
  /// **'Relation to review'**
  String get relationSuggestion;

  /// Task suggestion.
  ///
  /// In en, this message translates to:
  /// **'Task from a capture'**
  String get taskSuggestion;

  /// Filing suggestion.
  ///
  /// In en, this message translates to:
  /// **'Filing to review'**
  String get filingSuggestion;

  /// Unsupported suggestion.
  ///
  /// In en, this message translates to:
  /// **'Suggestion'**
  String get otherSuggestion;

  /// Accept a suggestion.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get accept;

  /// Reject a suggestion.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get dismiss;

  /// Create person from a link-or-create suggestion.
  ///
  /// In en, this message translates to:
  /// **'Create person…'**
  String get createPerson;

  /// AI confidence tag.
  ///
  /// In en, this message translates to:
  /// **'AI · {value}'**
  String aiConfidence({required String value});

  /// Create button.
  ///
  /// In en, this message translates to:
  /// **'New person'**
  String get newPerson;

  /// Create button.
  ///
  /// In en, this message translates to:
  /// **'New company'**
  String get newCompany;

  /// Create dialog field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get nameField;

  /// Create dialog field.
  ///
  /// In en, this message translates to:
  /// **'Aliases (one per line)'**
  String get aliasesField;

  /// Create.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get create;

  /// Create despite duplicates.
  ///
  /// In en, this message translates to:
  /// **'Create anyway'**
  String get createAnyway;

  /// Cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// Duplicate prompt title.
  ///
  /// In en, this message translates to:
  /// **'Already exists'**
  String get alreadyExists;

  /// Candidate match.
  ///
  /// In en, this message translates to:
  /// **'{level} · {score}'**
  String matchScore({required String level, required String score});

  /// Open the duplicate.
  ///
  /// In en, this message translates to:
  /// **'Open existing'**
  String get openExisting;

  /// Column.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get colName;

  /// Column.
  ///
  /// In en, this message translates to:
  /// **'Aliases'**
  String get colAliases;

  /// Column.
  ///
  /// In en, this message translates to:
  /// **'Details'**
  String get colDetails;

  /// Table caption.
  ///
  /// In en, this message translates to:
  /// **'{tab}, by name'**
  String tableCaption({required String tab});

  /// Keyboard hint after the arrow keys.
  ///
  /// In en, this message translates to:
  /// **'move · open page'**
  String get keyboardHint;

  /// Preview panel title.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get preview;

  /// Preview panel label.
  ///
  /// In en, this message translates to:
  /// **'Preview: {title}'**
  String previewOf({required String title});

  /// Close preview.
  ///
  /// In en, this message translates to:
  /// **'Close preview'**
  String get closePreview;

  /// Open the entity page.
  ///
  /// In en, this message translates to:
  /// **'Open page'**
  String get openPage;

  /// Merge action.
  ///
  /// In en, this message translates to:
  /// **'Merge…'**
  String get merge;

  /// Asks the AI to re-read the notes.
  ///
  /// In en, this message translates to:
  /// **'Refresh insights'**
  String get refreshInsights;

  /// Snack bar after refresh.
  ///
  /// In en, this message translates to:
  /// **'Insights refresh queued'**
  String get refreshQueued;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Summary'**
  String get summary;

  /// Section caption.
  ///
  /// In en, this message translates to:
  /// **'AI-maintained'**
  String get aiMaintained;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Insights'**
  String get insights;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Open items'**
  String get openItems;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Timeline'**
  String get timeline;

  /// Empty summary.
  ///
  /// In en, this message translates to:
  /// **'No summary yet.'**
  String get noSummary;

  /// Empty AI section.
  ///
  /// In en, this message translates to:
  /// **'Nothing yet.'**
  String get nothingYet;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Related entities'**
  String get relatedEntities;

  /// Empty related.
  ///
  /// In en, this message translates to:
  /// **'No related people or companies yet.'**
  String get noRelated;

  /// Reject button label.
  ///
  /// In en, this message translates to:
  /// **'Reject relation {type} {title}'**
  String rejectRelation({required String type, required String title});

  /// Repoint button label.
  ///
  /// In en, this message translates to:
  /// **'Repoint {title}'**
  String repointRelation({required String title});

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Mentioning notes'**
  String get mentioningNotes;

  /// Caption.
  ///
  /// In en, this message translates to:
  /// **'Newest first'**
  String get newestFirst;

  /// Empty mentions.
  ///
  /// In en, this message translates to:
  /// **'No notes mention this yet.'**
  String get noMentions;

  /// Section: documents held / owned.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get documentsSection;

  /// Section.
  ///
  /// In en, this message translates to:
  /// **'Entity graph'**
  String get entityGraph;

  /// User section.
  ///
  /// In en, this message translates to:
  /// **'Your notes'**
  String get yourNotes;

  /// User section caption.
  ///
  /// In en, this message translates to:
  /// **'Only you edit this section · ## Notes'**
  String get yourNotesHint;

  /// Entity tab.
  ///
  /// In en, this message translates to:
  /// **'Overview'**
  String get tabOverview;

  /// Entity tab.
  ///
  /// In en, this message translates to:
  /// **'Notes · {count}'**
  String tabNotes({required int count});

  /// Entity tab.
  ///
  /// In en, this message translates to:
  /// **'Graph'**
  String get tabGraph;

  /// Entity tab bar label.
  ///
  /// In en, this message translates to:
  /// **'Entity views'**
  String get entityViews;

  /// Kind.
  ///
  /// In en, this message translates to:
  /// **'Person'**
  String get kindPerson;

  /// Kind.
  ///
  /// In en, this message translates to:
  /// **'Company'**
  String get kindCompany;

  /// Entity header subtitle.
  ///
  /// In en, this message translates to:
  /// **'{kind} · {count, plural, =0{no mentioning notes} =1{1 mentioning note} other{{count} mentioning notes}}'**
  String entitySubtitle({required String kind, required int count});

  /// Back button.
  ///
  /// In en, this message translates to:
  /// **'Back to People'**
  String get backToPeople;

  /// Back button.
  ///
  /// In en, this message translates to:
  /// **'Back to Companies'**
  String get backToCompanies;

  /// Menu item.
  ///
  /// In en, this message translates to:
  /// **'Open markdown file'**
  String get openNote;

  /// Local changes.
  ///
  /// In en, this message translates to:
  /// **'Not synced yet'**
  String get pendingSync;

  /// Loading.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// Error title.
  ///
  /// In en, this message translates to:
  /// **'This couldn\'t be loaded'**
  String get errorTitle;

  /// Error body.
  ///
  /// In en, this message translates to:
  /// **'The app\'s local data returned an error ({code}).'**
  String errorMessage({required String code});

  /// Unknown entity.
  ///
  /// In en, this message translates to:
  /// **'This page isn\'t in the vault any more'**
  String get notFoundTitle;

  /// Unknown entity body.
  ///
  /// In en, this message translates to:
  /// **'It may have been deleted or merged on another device.'**
  String get notFoundMessage;

  /// Empty detail pane.
  ///
  /// In en, this message translates to:
  /// **'Select an item to see it here'**
  String get selectSomething;

  /// Tooltip of the directory sort menu.
  ///
  /// In en, this message translates to:
  /// **'Sort'**
  String get sortBy;

  /// Sort by name.
  ///
  /// In en, this message translates to:
  /// **'Name A–Z'**
  String get sortName;

  /// Sort by last activity.
  ///
  /// In en, this message translates to:
  /// **'Last active'**
  String get sortLastActive;

  /// Documents sorted by their latest move.
  ///
  /// In en, this message translates to:
  /// **'Recently moved'**
  String get sortRecentlyMoved;

  /// A filter chip: the core's label and the matching row count.
  ///
  /// In en, this message translates to:
  /// **'{label} · {count}'**
  String filterOption({required String label, required int count});

  /// Directory row activity: mention count and the core's last-active label.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No mentions} =1{1 mention} other{{count} mentions}} · {when}'**
  String rowActivity({required int count, required String when});

  /// Directory table column: mentions and last activity.
  ///
  /// In en, this message translates to:
  /// **'Activity'**
  String get colActivity;

  /// How many notes mention the entity.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No mentions} =1{1 mention} other{{count} mentions}}'**
  String mentionCount({required int count});

  /// Label of the entity picker search field.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get pickerSearch;

  /// Title of the repoint picker.
  ///
  /// In en, this message translates to:
  /// **'Point {title} to…'**
  String repointTitle({required String title});

  /// Snack bar for a failed directory action.
  ///
  /// In en, this message translates to:
  /// **'That didn\'t work ({code}).'**
  String actionFailed({required String code});

  /// Title of the merge target picker.
  ///
  /// In en, this message translates to:
  /// **'Merge {title} into…'**
  String mergeInto({required String title});

  /// Title of the merge confirmation.
  ///
  /// In en, this message translates to:
  /// **'Merge pages'**
  String get mergeTitle;

  /// Merge confirmation body.
  ///
  /// In en, this message translates to:
  /// **'{source} will be merged into {into}.'**
  String mergeBody({required String source, required String into});

  /// What a merge moves (from the core's preview).
  ///
  /// In en, this message translates to:
  /// **'Moves {mentions, plural, =0{no mentions} =1{1 mention} other{{mentions} mentions}} and {relations, plural, =0{no relations} =1{1 relation} other{{relations} relations}}.'**
  String mergeMoves({required int mentions, required int relations});

  /// Heading of the aliases a merge adds.
  ///
  /// In en, this message translates to:
  /// **'Aliases added'**
  String get mergeAliases;

  /// Confirms a merge.
  ///
  /// In en, this message translates to:
  /// **'Merge'**
  String get mergeConfirm;

  /// Snack bar after a merge.
  ///
  /// In en, this message translates to:
  /// **'Merged'**
  String get merged;

  /// Adds an alias.
  ///
  /// In en, this message translates to:
  /// **'Add alias'**
  String get addAlias;

  /// Label of the alias field.
  ///
  /// In en, this message translates to:
  /// **'Alias'**
  String get aliasField;

  /// Confirms adding.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get add;

  /// Adds a property.
  ///
  /// In en, this message translates to:
  /// **'Add property'**
  String get addProperty;

  /// Title of the property editor.
  ///
  /// In en, this message translates to:
  /// **'Edit {key}'**
  String editProperty({required String key});

  /// Label of the property name field.
  ///
  /// In en, this message translates to:
  /// **'Property'**
  String get propertyKey;

  /// Label of the property value field.
  ///
  /// In en, this message translates to:
  /// **'Value'**
  String get propertyValue;

  /// Saves a property.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// Tooltip of a property's menu.
  ///
  /// In en, this message translates to:
  /// **'{key} actions'**
  String propertyActions({required String key});

  /// Menu item: edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// Menu item: remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// Tooltip of an alias chip's delete button.
  ///
  /// In en, this message translates to:
  /// **'Remove alias {alias}'**
  String removeAlias({required String alias});

  /// Entity header subtitle with the core's last-active label.
  ///
  /// In en, this message translates to:
  /// **'{subtitle} · active {when}'**
  String entitySubtitleActive({required String subtitle, required String when});

  /// Caption of an AI section with its update time.
  ///
  /// In en, this message translates to:
  /// **'AI-maintained · updated {when}'**
  String aiUpdated({required String when});

  /// Caption of Open items: counts from the core.
  ///
  /// In en, this message translates to:
  /// **'{open} open · {done} done'**
  String openDone({required int open, required int done});

  /// Snack bar when creating from a who-is suggestion finds a duplicate.
  ///
  /// In en, this message translates to:
  /// **'Already exists: {title}'**
  String createExists({required String title});

  /// Who-is answer linking the mention to a candidate.
  ///
  /// In en, this message translates to:
  /// **'It\'s {title}'**
  String linkTo({required String title});

  /// Who-is answer creating a company.
  ///
  /// In en, this message translates to:
  /// **'Create company…'**
  String get createCompany;
}

class _DirectoryLocalizationsDelegate
    extends LocalizationsDelegate<DirectoryLocalizations> {
  const _DirectoryLocalizationsDelegate();

  @override
  Future<DirectoryLocalizations> load(Locale locale) {
    return SynchronousFuture<DirectoryLocalizations>(
      lookupDirectoryLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_DirectoryLocalizationsDelegate old) => false;
}

DirectoryLocalizations lookupDirectoryLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return DirectoryLocalizationsAr();
    case 'en':
      return DirectoryLocalizationsEn();
  }

  throw FlutterError(
    'DirectoryLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
