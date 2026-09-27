import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'strata_localizations_ar.dart';
import 'strata_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of StrataLocalizations
/// returned by `StrataLocalizations.of(context)`.
///
/// Applications need to include `StrataLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/strata_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: StrataLocalizations.localizationsDelegates,
///   supportedLocales: StrataLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the StrataLocalizations.supportedLocales
/// property.
abstract class StrataLocalizations {
  StrataLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static StrataLocalizations of(BuildContext context) {
    return Localizations.of<StrataLocalizations>(context, StrataLocalizations)!;
  }

  static const LocalizationsDelegate<StrataLocalizations> delegate =
      _StrataLocalizationsDelegate();

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

  /// Product name (L17). The Latin wordmark is never translated; this is the window/app title.
  ///
  /// In en, this message translates to:
  /// **'Strata'**
  String get appTitle;

  /// Accessibility label of the main navigation region (bar, rail or sidebar).
  ///
  /// In en, this message translates to:
  /// **'Main navigation'**
  String get navigationLabel;

  /// Navigation destination: Home / capture.
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get navHome;

  /// Navigation destination: Inbox of captures with AI filing suggestions.
  ///
  /// In en, this message translates to:
  /// **'Inbox'**
  String get navInbox;

  /// Navigation destination: Tasks (rail and sidebar only; on compact tasks live on Home).
  ///
  /// In en, this message translates to:
  /// **'Tasks'**
  String get navTasks;

  /// Navigation destination: Notes list.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get navNotes;

  /// Navigation destination: global map (medium and expanded only).
  ///
  /// In en, this message translates to:
  /// **'Map'**
  String get navMap;

  /// Navigation destination: People, Companies, Documents and Places.
  ///
  /// In en, this message translates to:
  /// **'Directory'**
  String get navDirectory;

  /// Navigation destination: Ask (chat over the vault).
  ///
  /// In en, this message translates to:
  /// **'Ask'**
  String get navAsk;

  /// Navigation destination: Settings (bottom of rail and sidebar).
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get navSettings;

  /// Primary button in the sidebar that starts a new capture.
  ///
  /// In en, this message translates to:
  /// **'New capture'**
  String get actionNewCapture;

  /// Short label of the capture button in the navigation rail.
  ///
  /// In en, this message translates to:
  /// **'Capture'**
  String get actionCapture;

  /// Search action (icon button tooltip and label).
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get actionSearch;

  /// Common action: save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get actionSave;

  /// Common action: cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get actionCancel;

  /// Common action: accept an AI suggestion.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get actionAccept;

  /// Common action: reject an AI suggestion.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get actionReject;

  /// Common action: edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get actionEdit;

  /// Common action: delete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get actionDelete;

  /// Common action: open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get actionOpen;

  /// Common action: go back.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get actionBack;

  /// Common action: close a panel, sheet or dialog.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get actionClose;

  /// Common action: retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get actionRetry;

  /// Common action: start a sync.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get actionSyncNow;

  /// Common action: sign out.
  ///
  /// In en, this message translates to:
  /// **'Sign out'**
  String get actionSignOut;

  /// Ask: save the answer as a note.
  ///
  /// In en, this message translates to:
  /// **'Save as note'**
  String get actionSaveAsNote;

  /// Label of the context panel (backlinks, relations, history).
  ///
  /// In en, this message translates to:
  /// **'Context'**
  String get contextPanelLabel;

  /// Sync pill: everything is synced; time of the last sync.
  ///
  /// In en, this message translates to:
  /// **'Synced · {time}'**
  String syncSynced({required String time});

  /// Sync pill: device is offline with queued outbox changes.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Offline} =1{Offline · 1 change queued} other{Offline · {count} changes queued}}'**
  String syncOfflineQueued({required int count});

  /// Sync pill: sync in progress.
  ///
  /// In en, this message translates to:
  /// **'Syncing {done}/{total}'**
  String syncSyncing({required int done, required int total});

  /// Sync pill: unresolved sync conflicts.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 conflict} other{{count} conflicts}}'**
  String syncConflicts({required int count});

  /// Accessibility hint of the sync pill button.
  ///
  /// In en, this message translates to:
  /// **'Sync status'**
  String get syncStatusLabel;

  /// Relation type name shown on typed relation chips and graph legends: related.
  ///
  /// In en, this message translates to:
  /// **'related'**
  String get relationRelated;

  /// Relation type name shown on typed relation chips and graph legends: part of.
  ///
  /// In en, this message translates to:
  /// **'part of'**
  String get relationPartOf;

  /// Relation type name shown on typed relation chips and graph legends: supports.
  ///
  /// In en, this message translates to:
  /// **'supports'**
  String get relationSupports;

  /// Relation type name shown on typed relation chips and graph legends: contradicts.
  ///
  /// In en, this message translates to:
  /// **'contradicts'**
  String get relationContradicts;

  /// Relation type name shown on typed relation chips and graph legends: follows up.
  ///
  /// In en, this message translates to:
  /// **'follows up'**
  String get relationFollowsUp;

  /// Relation type name shown on typed relation chips and graph legends: duplicates.
  ///
  /// In en, this message translates to:
  /// **'duplicates'**
  String get relationDuplicates;

  /// Relation type name shown on typed relation chips and graph legends: similar.
  ///
  /// In en, this message translates to:
  /// **'similar'**
  String get relationSimilarity;

  /// Relation type name shown on typed relation chips and graph legends: link.
  ///
  /// In en, this message translates to:
  /// **'link'**
  String get relationBodyLink;

  /// Relation type name shown on typed relation chips and graph legends: mentions.
  ///
  /// In en, this message translates to:
  /// **'mentions'**
  String get relationMention;

  /// Tag on AI-made relations and suggestions.
  ///
  /// In en, this message translates to:
  /// **'AI'**
  String get aiTag;

  /// Accessibility text for an AI-made relation with its confidence.
  ///
  /// In en, this message translates to:
  /// **'Suggested by AI, confidence {confidence}'**
  String aiConfidenceSemantics({required String confidence});

  /// Graph node kind name: Note.
  ///
  /// In en, this message translates to:
  /// **'Note'**
  String get nodeKindNote;

  /// Graph node kind name: Concept.
  ///
  /// In en, this message translates to:
  /// **'Concept'**
  String get nodeKindConcept;

  /// Graph node kind name: Person.
  ///
  /// In en, this message translates to:
  /// **'Person'**
  String get nodeKindPerson;

  /// Graph node kind name: Company.
  ///
  /// In en, this message translates to:
  /// **'Company'**
  String get nodeKindCompany;

  /// Graph node kind name: Document.
  ///
  /// In en, this message translates to:
  /// **'Document'**
  String get nodeKindDocument;

  /// Graph node kind name: Place.
  ///
  /// In en, this message translates to:
  /// **'Place'**
  String get nodeKindPlace;

  /// Accessibility label of a citation chip that opens the cited block.
  ///
  /// In en, this message translates to:
  /// **'Source: {label}'**
  String citationSemantics({required String label});

  /// Accessibility label of a keyboard-hint chip.
  ///
  /// In en, this message translates to:
  /// **'Keyboard shortcut {keys}'**
  String shortcutSemantics({required String keys});

  /// Message on placeholder feature screens until the Rust core view-models exist.
  ///
  /// In en, this message translates to:
  /// **'This screen is being built.'**
  String get featurePlaceholderMessage;

  /// Title of the Note editor screen.
  ///
  /// In en, this message translates to:
  /// **'Note editor'**
  String get featureEditor;

  /// Title of the Documents screen.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get featureDocuments;

  /// Title of the Sync & conflicts screen.
  ///
  /// In en, this message translates to:
  /// **'Sync & conflicts'**
  String get featureSync;

  /// Title of the Sign in screen.
  ///
  /// In en, this message translates to:
  /// **'Sign in'**
  String get featureAccounts;

  /// Title of the Users screen.
  ///
  /// In en, this message translates to:
  /// **'Users'**
  String get featureAdmin;
}

class _StrataLocalizationsDelegate
    extends LocalizationsDelegate<StrataLocalizations> {
  const _StrataLocalizationsDelegate();

  @override
  Future<StrataLocalizations> load(Locale locale) {
    return SynchronousFuture<StrataLocalizations>(
      lookupStrataLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_StrataLocalizationsDelegate old) => false;
}

StrataLocalizations lookupStrataLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return StrataLocalizationsAr();
    case 'en':
      return StrataLocalizationsEn();
  }

  throw FlutterError(
    'StrataLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
