import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'ask_localizations_ar.dart';
import 'ask_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AskLocalizations
/// returned by `AskLocalizations.of(context)`.
///
/// Applications need to include `AskLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/ask_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AskLocalizations.localizationsDelegates,
///   supportedLocales: AskLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the AskLocalizations.supportedLocales
/// property.
abstract class AskLocalizations {
  AskLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AskLocalizations of(BuildContext context) {
    return Localizations.of<AskLocalizations>(context, AskLocalizations)!;
  }

  static const LocalizationsDelegate<AskLocalizations> delegate =
      _AskLocalizationsDelegate();

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

  /// Ask title.
  ///
  /// In en, this message translates to:
  /// **'Ask'**
  String get askTitle;

  /// Conversation region label.
  ///
  /// In en, this message translates to:
  /// **'Conversation'**
  String get conversation;

  /// New conversation.
  ///
  /// In en, this message translates to:
  /// **'New conversation'**
  String get newConversation;

  /// Offline banner.
  ///
  /// In en, this message translates to:
  /// **'Ask needs a connection'**
  String get offlineTitle;

  /// Offline banner body.
  ///
  /// In en, this message translates to:
  /// **'Your notes are still searchable offline. Ask works again when you are back online.'**
  String get offlineMessage;

  /// Not-yet banner.
  ///
  /// In en, this message translates to:
  /// **'Ask isn\'t available on this server yet'**
  String get notYetTitle;

  /// Not-yet banner body.
  ///
  /// In en, this message translates to:
  /// **'Answers with citations will appear here once the server supports Ask.'**
  String get notYetMessage;

  /// Not-allowed banner.
  ///
  /// In en, this message translates to:
  /// **'Ask isn\'t available for this account'**
  String get notAllowedTitle;

  /// Empty conversation.
  ///
  /// In en, this message translates to:
  /// **'Ask about your notes'**
  String get emptyTitle;

  /// Empty conversation body.
  ///
  /// In en, this message translates to:
  /// **'Answers cite the notes they come from, in English or Arabic.'**
  String get emptyMessage;

  /// User message label.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get you;

  /// Assistant message label.
  ///
  /// In en, this message translates to:
  /// **'Answer'**
  String get answer;

  /// Sources heading.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No sources} =1{Sources · 1} other{Sources · {count}}}'**
  String sources({required int count});

  /// Save the answer.
  ///
  /// In en, this message translates to:
  /// **'Save as note'**
  String get saveAsNote;

  /// Copy.
  ///
  /// In en, this message translates to:
  /// **'Copy answer'**
  String get copyAnswer;

  /// Snack bar.
  ///
  /// In en, this message translates to:
  /// **'Answer copied'**
  String get copied;

  /// Composer label.
  ///
  /// In en, this message translates to:
  /// **'Question'**
  String get question;

  /// Composer placeholder.
  ///
  /// In en, this message translates to:
  /// **'Ask about your notes — English or العربية'**
  String get askHint;

  /// Question field hint when the conversation is about one note.
  ///
  /// In en, this message translates to:
  /// **'Ask about this note'**
  String get askAboutNoteHint;

  /// Removes the note from the conversation (a new conversation about the whole vault).
  ///
  /// In en, this message translates to:
  /// **'Ask about all notes'**
  String get leaveNoteThread;

  /// Send.
  ///
  /// In en, this message translates to:
  /// **'Send'**
  String get send;

  /// Scope selector label.
  ///
  /// In en, this message translates to:
  /// **'Scope'**
  String get scope;

  /// Preview panel title.
  ///
  /// In en, this message translates to:
  /// **'Source preview'**
  String get sourcePreview;

  /// Empty preview.
  ///
  /// In en, this message translates to:
  /// **'Select a citation to preview its note'**
  String get sourcePreviewHint;

  /// Opens the cited block.
  ///
  /// In en, this message translates to:
  /// **'Open at block'**
  String get openAtBlock;

  /// Unresolved citation.
  ///
  /// In en, this message translates to:
  /// **'The cited note isn\'t on this device'**
  String get noteMissing;

  /// Search title.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get searchTitle;

  /// Search field label.
  ///
  /// In en, this message translates to:
  /// **'Search your notes'**
  String get searchField;

  /// Search placeholder.
  ///
  /// In en, this message translates to:
  /// **'Search in English or العربية'**
  String get searchHint;

  /// Mode selector.
  ///
  /// In en, this message translates to:
  /// **'Search mode'**
  String get searchMode;

  /// Mode.
  ///
  /// In en, this message translates to:
  /// **'Keyword'**
  String get modeKeyword;

  /// Mode.
  ///
  /// In en, this message translates to:
  /// **'Semantic'**
  String get modeSemantic;

  /// Mode.
  ///
  /// In en, this message translates to:
  /// **'Hybrid'**
  String get modeHybrid;

  /// Empty query.
  ///
  /// In en, this message translates to:
  /// **'Type to search'**
  String get searchPrompt;

  /// Empty query body.
  ///
  /// In en, this message translates to:
  /// **'Keyword search works offline; semantic and hybrid search need the server.'**
  String get searchPromptMessage;

  /// No results.
  ///
  /// In en, this message translates to:
  /// **'No results for “{query}”'**
  String noResults({required String query});

  /// No results body.
  ///
  /// In en, this message translates to:
  /// **'Try other words, or the other script.'**
  String get noResultsMessage;

  /// Mode offline.
  ///
  /// In en, this message translates to:
  /// **'{mode} search needs a connection — showing nothing until you are online.'**
  String modeOffline({required String mode});

  /// Mode not yet available.
  ///
  /// In en, this message translates to:
  /// **'{mode} search isn\'t available on this server yet.'**
  String modeNotYet({required String mode});

  /// Mode not allowed.
  ///
  /// In en, this message translates to:
  /// **'{mode} search isn\'t available for this account.'**
  String modeNotAllowed({required String mode});

  /// Result count.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 result} other{{count} results}}'**
  String resultsCount({required int count});

  /// Open.
  ///
  /// In en, this message translates to:
  /// **'Open note'**
  String get openNote;

  /// Preview panel.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get preview;

  /// Empty preview.
  ///
  /// In en, this message translates to:
  /// **'Select a result to preview it'**
  String get previewHint;

  /// Loading.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// Error.
  ///
  /// In en, this message translates to:
  /// **'This couldn\'t be loaded'**
  String get errorTitle;

  /// Error body.
  ///
  /// In en, this message translates to:
  /// **'The app\'s local data returned an error ({code}).'**
  String errorMessage({required String code});

  /// Stops the streaming answer.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop;

  /// Live region while an answer streams.
  ///
  /// In en, this message translates to:
  /// **'Answering…'**
  String get answering;

  /// Answer stopped by the user.
  ///
  /// In en, this message translates to:
  /// **'You stopped this answer.'**
  String get answerStopped;

  /// Answer stopped because AI is paused.
  ///
  /// In en, this message translates to:
  /// **'AI is paused — this answer stopped early.'**
  String get answerPaused;

  /// Answer stopped because AI is unavailable.
  ///
  /// In en, this message translates to:
  /// **'AI isn\'t reachable — this answer stopped early.'**
  String get answerUnavailable;

  /// Answer stopped for another reason.
  ///
  /// In en, this message translates to:
  /// **'This answer stopped early.'**
  String get answerFailed;

  /// Snack bar after saving an answer.
  ///
  /// In en, this message translates to:
  /// **'Saved as a note'**
  String get savedAsNote;

  /// Button of an answer already saved as a note.
  ///
  /// In en, this message translates to:
  /// **'Saved · Open note'**
  String get openSavedNote;

  /// Semantics label of an inline citation marker.
  ///
  /// In en, this message translates to:
  /// **'Citation {index}'**
  String citationMarker({required int index});

  /// A semantic or hybrid result's score.
  ///
  /// In en, this message translates to:
  /// **'Score {value}'**
  String score({required String value});
}

class _AskLocalizationsDelegate
    extends LocalizationsDelegate<AskLocalizations> {
  const _AskLocalizationsDelegate();

  @override
  Future<AskLocalizations> load(Locale locale) {
    return SynchronousFuture<AskLocalizations>(lookupAskLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AskLocalizationsDelegate old) => false;
}

AskLocalizations lookupAskLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return AskLocalizationsAr();
    case 'en':
      return AskLocalizationsEn();
  }

  throw FlutterError(
    'AskLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
