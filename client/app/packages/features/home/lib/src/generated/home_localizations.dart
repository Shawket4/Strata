import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'home_localizations_ar.dart';
import 'home_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of HomeLocalizations
/// returned by `HomeLocalizations.of(context)`.
///
/// Applications need to include `HomeLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/home_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: HomeLocalizations.localizationsDelegates,
///   supportedLocales: HomeLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the HomeLocalizations.supportedLocales
/// property.
abstract class HomeLocalizations {
  HomeLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static HomeLocalizations of(BuildContext context) {
    return Localizations.of<HomeLocalizations>(context, HomeLocalizations)!;
  }

  static const LocalizationsDelegate<HomeLocalizations> delegate =
      _HomeLocalizationsDelegate();

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

  /// Home heading (medium / expanded).
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get homeTitle;

  /// Home error title.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load Home'**
  String get homeLoadError;

  /// Capture composer label.
  ///
  /// In en, this message translates to:
  /// **'Capture a thought'**
  String get homeComposerLabel;

  /// Capture composer hint.
  ///
  /// In en, this message translates to:
  /// **'Type or dictate · Arabic or English'**
  String get homeComposerHint;

  /// Capture composer footer.
  ///
  /// In en, this message translates to:
  /// **'AI files it into your inbox; nothing is filed until you accept.'**
  String get homeComposerFooter;

  /// Capture composer footer while offline.
  ///
  /// In en, this message translates to:
  /// **'Offline · captures are kept on this device and sync later.'**
  String get homeComposerOffline;

  /// Label next to the Ctrl/Cmd+N key hint.
  ///
  /// In en, this message translates to:
  /// **'capture'**
  String get homeFocusHint;

  /// Saves the capture.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get homeSave;

  /// Snack bar after a capture.
  ///
  /// In en, this message translates to:
  /// **'Saved to Inbox'**
  String get homeCaptureSaved;

  /// Snack bar when a capture failed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the capture'**
  String get homeCaptureFailed;

  /// Snack bar with the core failure code.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the capture ({code})'**
  String homeCaptureFailedCode({required String code});

  /// Inbox count card.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Inbox is clear} =1{1 capture waiting for filing} other{{count} captures waiting for filing}}'**
  String homeInboxWaiting({required int count});

  /// Opens Tasks.
  ///
  /// In en, this message translates to:
  /// **'All tasks'**
  String get homeAllTasks;

  /// Opens the notes list.
  ///
  /// In en, this message translates to:
  /// **'All notes'**
  String get homeAllNotes;

  /// Compact Home without tasks.
  ///
  /// In en, this message translates to:
  /// **'No open tasks. Add one from a capture or with New task.'**
  String get homeNoOpenTasks;

  /// Today block without tasks.
  ///
  /// In en, this message translates to:
  /// **'Nothing due today.'**
  String get homeNothingToday;

  /// Recent notes section.
  ///
  /// In en, this message translates to:
  /// **'Recent notes'**
  String get homeRecentNotes;

  /// No recent notes.
  ///
  /// In en, this message translates to:
  /// **'No notes yet. Your first capture becomes one once you accept it.'**
  String get homeNoNotes;

  /// Recent notes column.
  ///
  /// In en, this message translates to:
  /// **'NOTE'**
  String get homeColumnNote;

  /// Recent notes column.
  ///
  /// In en, this message translates to:
  /// **'PATH'**
  String get homeColumnPath;

  /// Recent notes column.
  ///
  /// In en, this message translates to:
  /// **'EDITED'**
  String get homeColumnEdited;

  /// Semantics label of the secondary column.
  ///
  /// In en, this message translates to:
  /// **'Today, inbox and AI activity'**
  String get homeSecondaryLabel;

  /// AI activity feed title.
  ///
  /// In en, this message translates to:
  /// **'AI activity'**
  String get homeAiActivity;

  /// AI activity unavailable.
  ///
  /// In en, this message translates to:
  /// **'Relations the AI adds, contradictions it finds and automatic changes you can undo show here when the server is reachable.'**
  String get homeAiActivityUnavailable;

  /// Open items roll-up title.
  ///
  /// In en, this message translates to:
  /// **'Open items'**
  String get homeOpenItems;

  /// Open items roll-up not provided yet.
  ///
  /// In en, this message translates to:
  /// **'Open items from people and company pages will be rolled up here.'**
  String get homeOpenItemsUnavailable;

  /// State of a block the core does not provide yet.
  ///
  /// In en, this message translates to:
  /// **'Not available yet'**
  String get homeNotYetAvailable;

  /// Inbox captures that need a decision.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 needs you} other{{count} need you}}'**
  String homeNeedsYou({required int count});

  /// Contradictions the AI found.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 contradiction} other{{count} contradictions}}'**
  String homeContradictions({required int count});

  /// Links of a recent note.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 link} other{{count} links}}'**
  String homeLinks({required int count});

  /// Label of the recent-notes filter.
  ///
  /// In en, this message translates to:
  /// **'Show'**
  String get homeRecentFilter;

  /// Recent notes: most recently edited.
  ///
  /// In en, this message translates to:
  /// **'Edited'**
  String get homeRecentEdited;

  /// Recent notes: most recently created.
  ///
  /// In en, this message translates to:
  /// **'Created'**
  String get homeRecentCreated;

  /// Recent notes: filed by the AI.
  ///
  /// In en, this message translates to:
  /// **'Filed by AI'**
  String get homeRecentFiledByAi;

  /// Snack bar for a failed Home action.
  ///
  /// In en, this message translates to:
  /// **'That didn\'t work ({code}).'**
  String homeActionFailed({required String code});

  /// A Home block that needs the server while offline.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get homeOffline;

  /// A Home block this account cannot use.
  ///
  /// In en, this message translates to:
  /// **'Not available for this account'**
  String get homeNotAllowed;

  /// Title of the relation type picker.
  ///
  /// In en, this message translates to:
  /// **'Change relation type'**
  String get homeRetypeTitle;

  /// AI confidence.
  ///
  /// In en, this message translates to:
  /// **'AI · {value}'**
  String homeAiConfidence({required String value});

  /// An AI decision already undone.
  ///
  /// In en, this message translates to:
  /// **'Undone'**
  String get homeAiUndone;

  /// Retypes an AI relation.
  ///
  /// In en, this message translates to:
  /// **'Change type'**
  String get homeAiRetype;

  /// Undoes an AI decision.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get homeAiUndo;

  /// Empty AI activity feed.
  ///
  /// In en, this message translates to:
  /// **'Nothing new from the AI.'**
  String get homeAiActivityEmpty;

  /// Empty open items.
  ///
  /// In en, this message translates to:
  /// **'No open items.'**
  String get homeOpenItemsEmpty;
}

class _HomeLocalizationsDelegate
    extends LocalizationsDelegate<HomeLocalizations> {
  const _HomeLocalizationsDelegate();

  @override
  Future<HomeLocalizations> load(Locale locale) {
    return SynchronousFuture<HomeLocalizations>(
      lookupHomeLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_HomeLocalizationsDelegate old) => false;
}

HomeLocalizations lookupHomeLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return HomeLocalizationsAr();
    case 'en':
      return HomeLocalizationsEn();
  }

  throw FlutterError(
    'HomeLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
