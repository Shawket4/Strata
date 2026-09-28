import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'notes_localizations_ar.dart';
import 'notes_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of NotesLocalizations
/// returned by `NotesLocalizations.of(context)`.
///
/// Applications need to include `NotesLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/notes_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: NotesLocalizations.localizationsDelegates,
///   supportedLocales: NotesLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the NotesLocalizations.supportedLocales
/// property.
abstract class NotesLocalizations {
  NotesLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static NotesLocalizations of(BuildContext context) {
    return Localizations.of<NotesLocalizations>(context, NotesLocalizations)!;
  }

  static const LocalizationsDelegate<NotesLocalizations> delegate =
      _NotesLocalizationsDelegate();

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

  /// Breadcrumb root: the vault's top folder.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get notesRoot;

  /// Semantics label of the folder breadcrumb.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get folderBreadcrumbLabel;

  /// Number of notes directly in a folder.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 note} other{{count} notes}}'**
  String folderNoteCount({required int count});

  /// Semantics label of a folder row.
  ///
  /// In en, this message translates to:
  /// **'Folder {name}, {count}'**
  String folderRowSemantics({required String name, required String count});

  /// Label and hint of the notes search field.
  ///
  /// In en, this message translates to:
  /// **'Search notes'**
  String get searchNotesLabel;

  /// Button that clears the notes search.
  ///
  /// In en, this message translates to:
  /// **'Clear search'**
  String get clearSearch;

  /// Semantics label of the notes list pane.
  ///
  /// In en, this message translates to:
  /// **'Notes in {folder}'**
  String notesInFolderLabel({required String folder});

  /// Empty state of a folder.
  ///
  /// In en, this message translates to:
  /// **'No notes here yet'**
  String get emptyFolderTitle;

  /// Empty state message of a folder.
  ///
  /// In en, this message translates to:
  /// **'Captures filed into this folder and notes you create here show up in this list.'**
  String get emptyFolderMessage;

  /// Search returned nothing.
  ///
  /// In en, this message translates to:
  /// **'No notes match “{query}”'**
  String noSearchResults({required String query});

  /// Search could not run.
  ///
  /// In en, this message translates to:
  /// **'Search isn\'t available right now.'**
  String get searchUnavailable;

  /// Error state of the notes list.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load this folder'**
  String get listErrorTitle;

  /// Error state message of the notes list.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong while reading notes on this device.'**
  String get listErrorMessage;

  /// Semantics label of the pending-sync dot on a note row.
  ///
  /// In en, this message translates to:
  /// **'Not synced yet'**
  String get notSyncedYet;

  /// Detail pane when no note is selected.
  ///
  /// In en, this message translates to:
  /// **'Select a note'**
  String get selectNoteTitle;

  /// Detail pane message when no note is selected.
  ///
  /// In en, this message translates to:
  /// **'Pick a note from the list to read and edit it.'**
  String get selectNoteMessage;

  /// Semantics label of the compact Note / Links / History tabs.
  ///
  /// In en, this message translates to:
  /// **'Note views'**
  String get noteViewsLabel;

  /// Compact tab: the note.
  ///
  /// In en, this message translates to:
  /// **'Note'**
  String get tabNote;

  /// Compact tab: relations and backlinks.
  ///
  /// In en, this message translates to:
  /// **'Links'**
  String get tabLinks;

  /// Tab: version history.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get tabHistory;

  /// Context tab: backlinks.
  ///
  /// In en, this message translates to:
  /// **'Backlinks'**
  String get tabBacklinks;

  /// Context tab: local graph.
  ///
  /// In en, this message translates to:
  /// **'Graph'**
  String get tabGraph;

  /// Semantics label of the context panel.
  ///
  /// In en, this message translates to:
  /// **'Note context'**
  String get contextLabel;

  /// Title of the medium-size context drawer.
  ///
  /// In en, this message translates to:
  /// **'Context'**
  String get contextTitle;

  /// Button that opens the context panel.
  ///
  /// In en, this message translates to:
  /// **'Show context panel'**
  String get showContext;

  /// Button that closes the context panel.
  ///
  /// In en, this message translates to:
  /// **'Hide context panel'**
  String get hideContext;

  /// Button that opens the local mind map.
  ///
  /// In en, this message translates to:
  /// **'Open local mind map'**
  String get openLocalMap;

  /// Opens Ask with the conversation about this note (its saved thread).
  ///
  /// In en, this message translates to:
  /// **'Ask AI about this note'**
  String get askAboutNote;

  /// Overflow menu button.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get moreActions;

  /// Back button of the compact note page.
  ///
  /// In en, this message translates to:
  /// **'Back to notes'**
  String get backToNotes;

  /// Title of the properties panel.
  ///
  /// In en, this message translates to:
  /// **'Properties'**
  String get propertiesTitle;

  /// Collapsed properties summary.
  ///
  /// In en, this message translates to:
  /// **'{relations, plural, =0{No relations} =1{1 relation} other{{relations} relations}}'**
  String propertiesSummary({required int relations});

  /// Button that expands the compact properties panel.
  ///
  /// In en, this message translates to:
  /// **'Expand properties'**
  String get expandProperties;

  /// Button that collapses the compact properties panel.
  ///
  /// In en, this message translates to:
  /// **'Collapse properties'**
  String get collapseProperties;

  /// Properties row: typed relations.
  ///
  /// In en, this message translates to:
  /// **'Relations'**
  String get relationsLabel;

  /// Properties row: tags.
  ///
  /// In en, this message translates to:
  /// **'Tags'**
  String get tagsLabel;

  /// A tag chip (the tag as the core gives it), isolated so it keeps its own direction.
  ///
  /// In en, this message translates to:
  /// **'⁨#{tag}⁩'**
  String tagChip({required String tag});

  /// Title of the AI relation card.
  ///
  /// In en, this message translates to:
  /// **'Suggested by AI'**
  String get aiSuggestedBy;

  /// Confidence of an AI relation.
  ///
  /// In en, this message translates to:
  /// **'confidence {value}'**
  String aiConfidence({required String value});

  /// Relation type and target on the AI card.
  ///
  /// In en, this message translates to:
  /// **'{type} · {title}'**
  String relationLine({required String type, required String title});

  /// AI relation without a reason.
  ///
  /// In en, this message translates to:
  /// **'No reason recorded.'**
  String get noReason;

  /// Reject an AI relation.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get relationReject;

  /// Change a relation's type.
  ///
  /// In en, this message translates to:
  /// **'Retype'**
  String get relationRetype;

  /// Title of the retype chooser.
  ///
  /// In en, this message translates to:
  /// **'Change relation type'**
  String get retypeTitle;

  /// Semantics hint on AI relation chips (phones).
  ///
  /// In en, this message translates to:
  /// **'Long-press for reject or retype'**
  String get relationActionsHint;

  /// The relation's target does not resolve locally.
  ///
  /// In en, this message translates to:
  /// **'Not in this vault yet'**
  String get relationUnresolved;

  /// Semantics label of the grouped backlinks.
  ///
  /// In en, this message translates to:
  /// **'Backlinks by relation type'**
  String get backlinksByType;

  /// No backlinks.
  ///
  /// In en, this message translates to:
  /// **'No notes link here yet.'**
  String get noBacklinks;

  /// Title of the local graph section.
  ///
  /// In en, this message translates to:
  /// **'Local graph'**
  String get localGraphTitle;

  /// Opens the local mind map.
  ///
  /// In en, this message translates to:
  /// **'Open map'**
  String get openMap;

  /// Title of the history section.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get historyTitle;

  /// History unavailable offline.
  ///
  /// In en, this message translates to:
  /// **'History needs a connection. It\'s back when you\'re online.'**
  String get historyOffline;

  /// History not built yet.
  ///
  /// In en, this message translates to:
  /// **'History isn\'t available yet.'**
  String get historyNotYetAvailable;

  /// History not allowed.
  ///
  /// In en, this message translates to:
  /// **'History isn\'t available for this account.'**
  String get historyNotAllowed;

  /// Context menu: delete.
  ///
  /// In en, this message translates to:
  /// **'Delete note'**
  String get deleteNote;

  /// Delete confirmation title.
  ///
  /// In en, this message translates to:
  /// **'Delete “{title}”?'**
  String deleteNoteTitle({required String title});

  /// Delete confirmation message.
  ///
  /// In en, this message translates to:
  /// **'The note moves to the vault\'s trash.'**
  String get deleteNoteMessage;

  /// Keyboard shortcut description.
  ///
  /// In en, this message translates to:
  /// **'Search notes'**
  String get shortcutsSearch;

  /// Note meta: creation date label from the core.
  ///
  /// In en, this message translates to:
  /// **'Created {date}'**
  String metaCreated({required String date});

  /// Note meta: last edit label.
  ///
  /// In en, this message translates to:
  /// **'edited {date}'**
  String metaEdited({required String date});

  /// Note meta: last edit label and author.
  ///
  /// In en, this message translates to:
  /// **'edited {date} by {name}'**
  String metaEditedBy({required String date, required String name});

  /// Word count of the note.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 word} other{{count} words}}'**
  String wordCount({required int count});

  /// Pin toggle (not pinned).
  ///
  /// In en, this message translates to:
  /// **'Pin to sidebar'**
  String get pinNote;

  /// Pin toggle (pinned).
  ///
  /// In en, this message translates to:
  /// **'Unpin from sidebar'**
  String get unpinNote;

  /// The block a link or citation points to.
  ///
  /// In en, this message translates to:
  /// **'Linked block'**
  String get linkedBlockTitle;

  /// The anchor did not resolve.
  ///
  /// In en, this message translates to:
  /// **'That block isn\'t in this note anymore.'**
  String get linkedBlockMissing;

  /// Duplicate sync state banner.
  ///
  /// In en, this message translates to:
  /// **'This note may already exist'**
  String get duplicateBannerTitle;

  /// Duplicate banner body.
  ///
  /// In en, this message translates to:
  /// **'Creating it found a similar note. Choose whether to keep it.'**
  String get duplicateBannerMessage;

  /// Duplicate banner action.
  ///
  /// In en, this message translates to:
  /// **'Review'**
  String get duplicateReview;

  /// Backlinks tab with the core's count.
  ///
  /// In en, this message translates to:
  /// **'Backlinks {count}'**
  String backlinksTab({required int count});

  /// Compact Links tab with the core's count.
  ///
  /// In en, this message translates to:
  /// **'Links {count}'**
  String linksTab({required int count});

  /// History with no entries.
  ///
  /// In en, this message translates to:
  /// **'No versions yet.'**
  String get historyEmpty;

  /// History count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 version} other{All {count} versions}}'**
  String historyAll({required int count});

  /// History author: user.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get authorUser;

  /// History author: ai.
  ///
  /// In en, this message translates to:
  /// **'Strata AI'**
  String get authorAi;

  /// History author: system.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get authorSystem;

  /// History entry author and time.
  ///
  /// In en, this message translates to:
  /// **'{author} · {when}'**
  String historyWho({required String author, required String when});

  /// History: revert to this version.
  ///
  /// In en, this message translates to:
  /// **'Revert'**
  String get revert;

  /// History: view the diff of a version.
  ///
  /// In en, this message translates to:
  /// **'Changes'**
  String get viewChanges;

  /// Revert confirmation title.
  ///
  /// In en, this message translates to:
  /// **'Revert to {version}?'**
  String revertTitle({required String version});

  /// Revert confirmation body.
  ///
  /// In en, this message translates to:
  /// **'The note\'s text goes back to this version. The current text stays in History.'**
  String get revertBody;

  /// Diff dialog title.
  ///
  /// In en, this message translates to:
  /// **'{version} compared with now'**
  String diffTitle({required String version});

  /// Close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// Cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// An intent failed.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong ({code}).'**
  String noteFailed({required String code});

  /// AI confidence tag on a backlink.
  ///
  /// In en, this message translates to:
  /// **'AI · {value}'**
  String aiConfidenceTag({required String value});
}

class _NotesLocalizationsDelegate
    extends LocalizationsDelegate<NotesLocalizations> {
  const _NotesLocalizationsDelegate();

  @override
  Future<NotesLocalizations> load(Locale locale) {
    return SynchronousFuture<NotesLocalizations>(
      lookupNotesLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_NotesLocalizationsDelegate old) => false;
}

NotesLocalizations lookupNotesLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return NotesLocalizationsAr();
    case 'en':
      return NotesLocalizationsEn();
  }

  throw FlutterError(
    'NotesLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
