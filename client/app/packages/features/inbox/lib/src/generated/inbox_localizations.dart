import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'inbox_localizations_ar.dart';
import 'inbox_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of InboxLocalizations
/// returned by `InboxLocalizations.of(context)`.
///
/// Applications need to include `InboxLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/inbox_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: InboxLocalizations.localizationsDelegates,
///   supportedLocales: InboxLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the InboxLocalizations.supportedLocales
/// property.
abstract class InboxLocalizations {
  InboxLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static InboxLocalizations of(BuildContext context) {
    return Localizations.of<InboxLocalizations>(context, InboxLocalizations)!;
  }

  static const LocalizationsDelegate<InboxLocalizations> delegate =
      _InboxLocalizationsDelegate();

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

  /// Inbox list title.
  ///
  /// In en, this message translates to:
  /// **'Inbox'**
  String get inboxTitle;

  /// Inbox error title.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load the inbox'**
  String get inboxLoadError;

  /// Empty inbox title.
  ///
  /// In en, this message translates to:
  /// **'Inbox zero'**
  String get inboxEmptyTitle;

  /// Empty inbox message.
  ///
  /// In en, this message translates to:
  /// **'Captures you save land here, and the AI proposes where they go. Nothing is filed until you accept.'**
  String get inboxEmptyMessage;

  /// Accept a proposal.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get inboxAccept;

  /// Reject a proposal.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get inboxReject;

  /// Edit (opens the capture note).
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get inboxEdit;

  /// Cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get inboxCancel;

  /// Undo an automatic change.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get inboxUndo;

  /// Discard a duplicate capture.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get inboxDiscard;

  /// Dismiss an unsupported suggestion.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get inboxDismiss;

  /// Snack bar when an inbox action failed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t apply that change'**
  String get inboxActionFailed;

  /// Snack bar with the core failure code.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t apply that change ({code})'**
  String inboxActionFailedCode({required String code});

  /// AI confidence tag.
  ///
  /// In en, this message translates to:
  /// **'AI · {score}'**
  String inboxAiConfidence({required String score});

  /// Header of the AI proposal of a capture.
  ///
  /// In en, this message translates to:
  /// **'AI PROPOSAL'**
  String get inboxAiProposal;

  /// Capture without suggestions.
  ///
  /// In en, this message translates to:
  /// **'No proposal yet — the AI has not read this capture.'**
  String get inboxNoProposalYet;

  /// Number of suggestions of a capture.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 proposal} other{{count} proposals}}'**
  String inboxProposalCount({required int count});

  /// A proposed tag.
  ///
  /// In en, this message translates to:
  /// **'#{tag}'**
  String inboxTag({required String tag});

  /// Filing proposal section.
  ///
  /// In en, this message translates to:
  /// **'PEOPLE & COMPANIES'**
  String get inboxPeopleAndCompanies;

  /// Filing proposal footnote.
  ///
  /// In en, this message translates to:
  /// **'Accepting files it as a new note in {folder}'**
  String inboxCreatesNoteIn({required String folder});

  /// Rejects one proposed relation.
  ///
  /// In en, this message translates to:
  /// **'Reject relation'**
  String get inboxRejectRelation;

  /// A task proposal label.
  ///
  /// In en, this message translates to:
  /// **'Proposed task'**
  String get inboxTaskProposal;

  /// Badge of a suggestion that needs a decision.
  ///
  /// In en, this message translates to:
  /// **'Needs you'**
  String get inboxNeedsYou;

  /// Badge of an automatically applied custody change.
  ///
  /// In en, this message translates to:
  /// **'Applied automatically'**
  String get inboxAppliedAutomatically;

  /// Badge of a custody suggestion.
  ///
  /// In en, this message translates to:
  /// **'Needs you · custody'**
  String get inboxCustodySuggestion;

  /// Badge of a duplicate-flagged item.
  ///
  /// In en, this message translates to:
  /// **'Possible duplicate'**
  String get inboxPossibleDuplicate;

  /// Badge of a relation suggestion.
  ///
  /// In en, this message translates to:
  /// **'Relation suggestion'**
  String get inboxRelationSuggestion;

  /// Badge of a task suggestion.
  ///
  /// In en, this message translates to:
  /// **'Task suggestion'**
  String get inboxTaskSuggestion;

  /// Badge of a filing suggestion.
  ///
  /// In en, this message translates to:
  /// **'Filing proposal'**
  String get inboxFilingProposal;

  /// Badge of a suggestion kind this version cannot show.
  ///
  /// In en, this message translates to:
  /// **'Suggestion'**
  String get inboxUnsupportedBadge;

  /// Unsupported suggestion message.
  ///
  /// In en, this message translates to:
  /// **'This app version can’t show “{kind}” suggestions yet. Update the app to review it.'**
  String inboxUnsupported({required String kind});

  /// A rejected suggestion.
  ///
  /// In en, this message translates to:
  /// **'Rejected'**
  String get inboxRejected;

  /// Link-or-create question.
  ///
  /// In en, this message translates to:
  /// **'Who is “{mention}”?'**
  String inboxWhoIs({required String mention});

  /// Link-or-create with no candidates.
  ///
  /// In en, this message translates to:
  /// **'A nickname. No person in your vault matches it yet.'**
  String get inboxNoPersonMatches;

  /// Link-or-create with candidates.
  ///
  /// In en, this message translates to:
  /// **'Possible matches in your vault:'**
  String get inboxPossibleMatches;

  /// Link-or-create action.
  ///
  /// In en, this message translates to:
  /// **'Create person…'**
  String get inboxCreatePerson;

  /// Link-or-create alias note.
  ///
  /// In en, this message translates to:
  /// **'Accepting adds “{mention}” as an alias, so future mentions resolve.'**
  String inboxAliasNote({required String mention});

  /// Create person sheet title.
  ///
  /// In en, this message translates to:
  /// **'Create person'**
  String get inboxCreatePersonTitle;

  /// Person name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get inboxPersonName;

  /// Aliases section.
  ///
  /// In en, this message translates to:
  /// **'ALIASES'**
  String get inboxAliases;

  /// Create person action.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get inboxCreatePersonSave;

  /// Ambiguous custody suggestion question.
  ///
  /// In en, this message translates to:
  /// **'Which document?'**
  String get inboxWhichDocument;

  /// Semantics label of a capture card.
  ///
  /// In en, this message translates to:
  /// **'Capture'**
  String get inboxCaptureSemantics;

  /// Bulk bar label.
  ///
  /// In en, this message translates to:
  /// **'Bulk actions'**
  String get inboxBulkActions;

  /// Select all checkbox.
  ///
  /// In en, this message translates to:
  /// **'Select all captures'**
  String get inboxSelectAll;

  /// Number of selected captures.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{None selected} =1{1 selected} other{{count} selected}}'**
  String inboxSelectedCount({required int count});

  /// Clears the bulk selection.
  ///
  /// In en, this message translates to:
  /// **'Clear selection'**
  String get inboxClearSelection;

  /// Checkbox of one capture.
  ///
  /// In en, this message translates to:
  /// **'Select capture'**
  String get inboxSelectCapture;

  /// List section of standalone suggestions.
  ///
  /// In en, this message translates to:
  /// **'Suggestions'**
  String get inboxSuggestionsHeader;

  /// List section of captures.
  ///
  /// In en, this message translates to:
  /// **'Captures'**
  String get inboxCapturesHeader;

  /// Detail section of standalone suggestions.
  ///
  /// In en, this message translates to:
  /// **'Also needs you'**
  String get inboxAlsoNeedsYou;

  /// Detail section with the capture text.
  ///
  /// In en, this message translates to:
  /// **'CAPTURE'**
  String get inboxCaptureHeader;

  /// A relation suggestion summary.
  ///
  /// In en, this message translates to:
  /// **'{type} {target}'**
  String inboxRelationSummary({required String type, required String target});

  /// Shortcut legend label.
  ///
  /// In en, this message translates to:
  /// **'Keyboard shortcuts'**
  String get inboxShortcutsLabel;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'move'**
  String get inboxKeyMove;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'select'**
  String get inboxKeySelect;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'accept'**
  String get inboxKeyAccept;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'reject'**
  String get inboxKeyReject;

  /// Shortcut legend.
  ///
  /// In en, this message translates to:
  /// **'edit'**
  String get inboxKeyEdit;

  /// A capture ready to accept as proposed.
  ///
  /// In en, this message translates to:
  /// **'Ready'**
  String get inboxReady;

  /// Acknowledges an automatic change.
  ///
  /// In en, this message translates to:
  /// **'Looks right'**
  String get inboxLooksRight;

  /// Keeps two items that look like duplicates.
  ///
  /// In en, this message translates to:
  /// **'Keep both'**
  String get inboxKeepBoth;

  /// Accepts a duplicates suggestion: merges the pair.
  ///
  /// In en, this message translates to:
  /// **'Merge'**
  String get inboxMerge;

  /// A correction in the user's words.
  ///
  /// In en, this message translates to:
  /// **'You said: “{words}”'**
  String inboxYouSaid({required String words});

  /// The mention is a nickname.
  ///
  /// In en, this message translates to:
  /// **'Nickname'**
  String get inboxNickname;

  /// Links the mention to an entity.
  ///
  /// In en, this message translates to:
  /// **'It\'s {title}'**
  String inboxItIs({required String title});

  /// Creates a company for the mention.
  ///
  /// In en, this message translates to:
  /// **'Create company…'**
  String get inboxCreateCompany;

  /// The capture's words a custody event comes from.
  ///
  /// In en, this message translates to:
  /// **'“{quote}”'**
  String inboxQuote({required String quote});

  /// Where the document is after the event.
  ///
  /// In en, this message translates to:
  /// **'Then at {place}'**
  String inboxAfterAt({required String place});

  /// Who holds the document after the event.
  ///
  /// In en, this message translates to:
  /// **'Then with {person}'**
  String inboxAfterWith({required String person});

  /// Nobody holds it after the event.
  ///
  /// In en, this message translates to:
  /// **'Nobody has it · last with {person}'**
  String inboxAfterLastWith({required String person});

  /// An AI message of a suggestion thread.
  ///
  /// In en, this message translates to:
  /// **'AI · {when}'**
  String inboxThreadAi({required String when});

  /// The user's message of a suggestion thread.
  ///
  /// In en, this message translates to:
  /// **'You · {when}'**
  String inboxThreadYou({required String when});

  /// Opens the reply field of a suggestion.
  ///
  /// In en, this message translates to:
  /// **'Reply to the AI'**
  String get inboxReply;

  /// Label of the reply field.
  ///
  /// In en, this message translates to:
  /// **'Your reply'**
  String get inboxReplyField;

  /// Sends the reply.
  ///
  /// In en, this message translates to:
  /// **'Send reply'**
  String get inboxReplySend;

  /// Title of the proposal editor.
  ///
  /// In en, this message translates to:
  /// **'Edit before accepting'**
  String get inboxEditTitle;

  /// Task text field.
  ///
  /// In en, this message translates to:
  /// **'Task'**
  String get inboxEditTaskText;

  /// Note title field.
  ///
  /// In en, this message translates to:
  /// **'Note title'**
  String get inboxEditNoteTitle;

  /// Folder field.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get inboxEditFolder;

  /// Accepts the edited proposal.
  ///
  /// In en, this message translates to:
  /// **'Accept with changes'**
  String get inboxAcceptEdited;

  /// Inbox filter: all.
  ///
  /// In en, this message translates to:
  /// **'All · {count}'**
  String inboxFilterAll({required int count});

  /// Inbox filter: needs you.
  ///
  /// In en, this message translates to:
  /// **'Needs you · {count}'**
  String inboxFilterNeedsYou({required int count});

  /// Inbox filter: conflicts.
  ///
  /// In en, this message translates to:
  /// **'Conflicts · {count}'**
  String inboxFilterConflicts({required int count});

  /// Label of the inbox filters.
  ///
  /// In en, this message translates to:
  /// **'Show'**
  String get inboxFilterLabel;

  /// Accepts every ready capture.
  ///
  /// In en, this message translates to:
  /// **'Accept all ready · {count}'**
  String inboxAcceptAllReady({required int count});
}

class _InboxLocalizationsDelegate
    extends LocalizationsDelegate<InboxLocalizations> {
  const _InboxLocalizationsDelegate();

  @override
  Future<InboxLocalizations> load(Locale locale) {
    return SynchronousFuture<InboxLocalizations>(
      lookupInboxLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_InboxLocalizationsDelegate old) => false;
}

InboxLocalizations lookupInboxLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return InboxLocalizationsAr();
    case 'en':
      return InboxLocalizationsEn();
  }

  throw FlutterError(
    'InboxLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
