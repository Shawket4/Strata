import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'documents_localizations_ar.dart';
import 'documents_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of DocumentsLocalizations
/// returned by `DocumentsLocalizations.of(context)`.
///
/// Applications need to include `DocumentsLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/documents_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: DocumentsLocalizations.localizationsDelegates,
///   supportedLocales: DocumentsLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the DocumentsLocalizations.supportedLocales
/// property.
abstract class DocumentsLocalizations {
  DocumentsLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static DocumentsLocalizations of(BuildContext context) {
    return Localizations.of<DocumentsLocalizations>(
      context,
      DocumentsLocalizations,
    )!;
  }

  static const LocalizationsDelegate<DocumentsLocalizations> delegate =
      _DocumentsLocalizationsDelegate();

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

  /// Section / back target: documents.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get documents;

  /// Section / back target: places.
  ///
  /// In en, this message translates to:
  /// **'Places'**
  String get places;

  /// Back button of a compact document page.
  ///
  /// In en, this message translates to:
  /// **'Back to Directory'**
  String get backToDocuments;

  /// Back button of a compact place page.
  ///
  /// In en, this message translates to:
  /// **'Back to Places'**
  String get backToPlaces;

  /// More actions menu button.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get moreActions;

  /// Opens the context drawer (medium).
  ///
  /// In en, this message translates to:
  /// **'Show context'**
  String get showContext;

  /// Context panel label.
  ///
  /// In en, this message translates to:
  /// **'Context'**
  String get contextLabel;

  /// Breadcrumb navigation label.
  ///
  /// In en, this message translates to:
  /// **'Breadcrumb'**
  String get breadcrumb;

  /// Place breadcrumb label.
  ///
  /// In en, this message translates to:
  /// **'Location'**
  String get locationLabel;

  /// Unknown document or place.
  ///
  /// In en, this message translates to:
  /// **'This page isn\'t in the vault any more'**
  String get notFoundTitle;

  /// Unknown page body.
  ///
  /// In en, this message translates to:
  /// **'It may have been deleted or merged on another device.'**
  String get notFoundMessage;

  /// Loading state.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// Error state title.
  ///
  /// In en, this message translates to:
  /// **'This couldn\'t be loaded'**
  String get errorTitle;

  /// Error state body.
  ///
  /// In en, this message translates to:
  /// **'The app\'s local data returned an error ({code}).'**
  String errorMessage({required String code});

  /// A page with local changes.
  ///
  /// In en, this message translates to:
  /// **'Not synced yet'**
  String get pendingSync;

  /// Kind label.
  ///
  /// In en, this message translates to:
  /// **'Document'**
  String get documentKind;

  /// Kind label.
  ///
  /// In en, this message translates to:
  /// **'Place'**
  String get placeKind;

  /// Type and copy line.
  ///
  /// In en, this message translates to:
  /// **'{type} · {copy}'**
  String docSubtitle({required String type, required String copy});

  /// Expiry.
  ///
  /// In en, this message translates to:
  /// **'Expires {date}'**
  String expiresOn({required DateTime date});

  /// Property label.
  ///
  /// In en, this message translates to:
  /// **'Expires'**
  String get expiresLabel;

  /// Property label.
  ///
  /// In en, this message translates to:
  /// **'Renewal'**
  String get renewalLabel;

  /// Property label.
  ///
  /// In en, this message translates to:
  /// **'Reminder'**
  String get reminderLabel;

  /// Property label.
  ///
  /// In en, this message translates to:
  /// **'Type'**
  String get typeLabel;

  /// Property label: companies/people the document concerns.
  ///
  /// In en, this message translates to:
  /// **'Concerns'**
  String get concernsLabel;

  /// Property / section label.
  ///
  /// In en, this message translates to:
  /// **'Copies'**
  String get copiesLabel;

  /// Copies count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No copies} =1{1 copy} other{{count} copies}}'**
  String copiesCount({required int count});

  /// Placeholder for a value the app cannot show yet.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get notAvailableYet;

  /// Where-it-is card title.
  ///
  /// In en, this message translates to:
  /// **'Where it is'**
  String get whereItIs;

  /// No location recorded.
  ///
  /// In en, this message translates to:
  /// **'Location unknown'**
  String get locationUnknown;

  /// Current holder.
  ///
  /// In en, this message translates to:
  /// **'With {name}'**
  String withHolder({required String name});

  /// No current holder.
  ///
  /// In en, this message translates to:
  /// **'Nobody has it now'**
  String get nobodyHasIt;

  /// Precedes the last holder link.
  ///
  /// In en, this message translates to:
  /// **'· last with'**
  String get lastWith;

  /// Precedes the current holder link.
  ///
  /// In en, this message translates to:
  /// **'With'**
  String get withLabel;

  /// Custody event heading.
  ///
  /// In en, this message translates to:
  /// **'{date} · {kind}'**
  String custodyHeading({required DateTime date, required String kind});

  /// Record a move action.
  ///
  /// In en, this message translates to:
  /// **'Record a move'**
  String get recordMove;

  /// Custody section title.
  ///
  /// In en, this message translates to:
  /// **'Custody history'**
  String get custodyHistory;

  /// Custody section helper.
  ///
  /// In en, this message translates to:
  /// **'Location and holder change only through these events'**
  String get custodyHint;

  /// Custody count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No events} =1{1 event · newest first} other{{count} events · newest first}}'**
  String custodyCount({required int count});

  /// Empty custody.
  ///
  /// In en, this message translates to:
  /// **'No custody events recorded yet.'**
  String get noCustody;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Stored at'**
  String get custodyStoredAt;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Moved to'**
  String get custodyMovedTo;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Handed to'**
  String get custodyHandedTo;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Returned by'**
  String get custodyReturnedBy;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Sent to'**
  String get custodySentTo;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Received from'**
  String get custodyReceivedFrom;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Lost'**
  String get custodyLost;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Found'**
  String get custodyFound;

  /// Custody event type.
  ///
  /// In en, this message translates to:
  /// **'Destroyed'**
  String get custodyDestroyed;

  /// Document status.
  ///
  /// In en, this message translates to:
  /// **'Stored'**
  String get statusStored;

  /// Document status.
  ///
  /// In en, this message translates to:
  /// **'Checked out'**
  String get statusCheckedOut;

  /// Document status.
  ///
  /// In en, this message translates to:
  /// **'Third party'**
  String get statusThirdParty;

  /// Document status.
  ///
  /// In en, this message translates to:
  /// **'Lost'**
  String get statusLost;

  /// Document status.
  ///
  /// In en, this message translates to:
  /// **'Destroyed'**
  String get statusDestroyed;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Contract'**
  String get typeContract;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'ID'**
  String get typeId;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Licence'**
  String get typeLicence;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Deed'**
  String get typeDeed;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Invoice'**
  String get typeInvoice;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Certificate'**
  String get typeCertificate;

  /// Document type.
  ///
  /// In en, this message translates to:
  /// **'Other'**
  String get typeOther;

  /// Copy kind.
  ///
  /// In en, this message translates to:
  /// **'Original'**
  String get copyOriginal;

  /// Copy kind.
  ///
  /// In en, this message translates to:
  /// **'Certified copy'**
  String get copyCertified;

  /// Copy kind.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copyCopy;

  /// Copy kind.
  ///
  /// In en, this message translates to:
  /// **'Digital'**
  String get copyDigital;

  /// User-owned notes section.
  ///
  /// In en, this message translates to:
  /// **'Your notes'**
  String get yourNotes;

  /// User notes helper.
  ///
  /// In en, this message translates to:
  /// **'Only you edit this'**
  String get yourNotesHint;

  /// User notes placeholder until the core streams the section.
  ///
  /// In en, this message translates to:
  /// **'Editing your notes here isn\'t available yet. Open the note to edit its ## Notes section.'**
  String get yourNotesUnavailable;

  /// Opens the markdown note.
  ///
  /// In en, this message translates to:
  /// **'Open note'**
  String get openNote;

  /// Undo an AI custody change.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get undoAi;

  /// Graph section.
  ///
  /// In en, this message translates to:
  /// **'Graph'**
  String get graph;

  /// Places tree title.
  ///
  /// In en, this message translates to:
  /// **'Places'**
  String get placesTree;

  /// Tree accessibility label.
  ///
  /// In en, this message translates to:
  /// **'{place} and places inside it'**
  String placesTreeLabel({required String place});

  /// Empty sub-places.
  ///
  /// In en, this message translates to:
  /// **'No places inside'**
  String get noSubPlaces;

  /// Place without a parent.
  ///
  /// In en, this message translates to:
  /// **'Top level'**
  String get topLevelPlace;

  /// Place parent line.
  ///
  /// In en, this message translates to:
  /// **'part of {place}'**
  String partOf({required String place});

  /// Documents at a place incl. nested places.
  ///
  /// In en, this message translates to:
  /// **'Everything here'**
  String get everythingHere;

  /// Table caption.
  ///
  /// In en, this message translates to:
  /// **'Documents at {place}, including places inside it'**
  String everythingHereCaption({required String place});

  /// Documents count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No documents} =1{1 document} other{{count} documents}}'**
  String documentsCount({required int count});

  /// Empty place.
  ///
  /// In en, this message translates to:
  /// **'Nothing is stored here yet.'**
  String get noDocumentsHere;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Document'**
  String get colDocument;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Where'**
  String get colWhere;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Holder'**
  String get colHolder;

  /// Table column.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get colStatus;

  /// Place movements section.
  ///
  /// In en, this message translates to:
  /// **'Recent movements'**
  String get recentMovements;

  /// Empty movements.
  ///
  /// In en, this message translates to:
  /// **'No movements recorded here yet.'**
  String get noMovements;

  /// Aliases label.
  ///
  /// In en, this message translates to:
  /// **'Aliases'**
  String get aliases;

  /// Record-a-move subtitle.
  ///
  /// In en, this message translates to:
  /// **'{document} · now in {place}'**
  String recordMoveSubtitle({required String document, required String place});

  /// Record-a-move subtitle without location.
  ///
  /// In en, this message translates to:
  /// **'{document}'**
  String recordMoveSubtitleUnknown({required String document});

  /// Event type group.
  ///
  /// In en, this message translates to:
  /// **'What happened'**
  String get whatHappened;

  /// Move event.
  ///
  /// In en, this message translates to:
  /// **'Moved to'**
  String get eventMovedTo;

  /// Move event.
  ///
  /// In en, this message translates to:
  /// **'Handed to'**
  String get eventHandedTo;

  /// Move event.
  ///
  /// In en, this message translates to:
  /// **'Returned'**
  String get eventReturned;

  /// Move event.
  ///
  /// In en, this message translates to:
  /// **'Sent to third party'**
  String get eventSentTo;

  /// Move event.
  ///
  /// In en, this message translates to:
  /// **'Lost'**
  String get eventLost;

  /// Place picker label.
  ///
  /// In en, this message translates to:
  /// **'To place'**
  String get toPlace;

  /// Person picker label.
  ///
  /// In en, this message translates to:
  /// **'Holder after'**
  String get holderAfter;

  /// No holder.
  ///
  /// In en, this message translates to:
  /// **'Nobody'**
  String get nobody;

  /// Document picker label.
  ///
  /// In en, this message translates to:
  /// **'Document'**
  String get documentField;

  /// Date field label.
  ///
  /// In en, this message translates to:
  /// **'Date'**
  String get dateField;

  /// Date not chosen: today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get dateToday;

  /// Date picker button.
  ///
  /// In en, this message translates to:
  /// **'Choose a date'**
  String get pickDate;

  /// Note field.
  ///
  /// In en, this message translates to:
  /// **'Note (optional)'**
  String get noteField;

  /// Record-a-move footer.
  ///
  /// In en, this message translates to:
  /// **'Adds a cited event to ## Custody'**
  String get recordMoveFooter;

  /// Submit.
  ///
  /// In en, this message translates to:
  /// **'Record move'**
  String get recordMoveSubmit;

  /// Shown until the core has a custody intent.
  ///
  /// In en, this message translates to:
  /// **'Recording a move from the app isn\'t available yet.'**
  String get recordMoveUnavailable;

  /// Cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// Close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// Place picker search.
  ///
  /// In en, this message translates to:
  /// **'Search places'**
  String get searchPlaces;

  /// Person picker search.
  ///
  /// In en, this message translates to:
  /// **'Search people'**
  String get searchPeople;

  /// Placeholder until the core streams document mentions.
  ///
  /// In en, this message translates to:
  /// **'Mentioning notes will appear here.'**
  String get mentionsUnavailable;

  /// A custody date.
  ///
  /// In en, this message translates to:
  /// **'{date}'**
  String dateShort({required DateTime date});
}

class _DocumentsLocalizationsDelegate
    extends LocalizationsDelegate<DocumentsLocalizations> {
  const _DocumentsLocalizationsDelegate();

  @override
  Future<DocumentsLocalizations> load(Locale locale) {
    return SynchronousFuture<DocumentsLocalizations>(
      lookupDocumentsLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_DocumentsLocalizationsDelegate old) => false;
}

DocumentsLocalizations lookupDocumentsLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return DocumentsLocalizationsAr();
    case 'en':
      return DocumentsLocalizationsEn();
  }

  throw FlutterError(
    'DocumentsLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
