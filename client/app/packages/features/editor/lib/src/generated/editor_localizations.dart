import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'editor_localizations_ar.dart';
import 'editor_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of EditorLocalizations
/// returned by `EditorLocalizations.of(context)`.
///
/// Applications need to include `EditorLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/editor_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: EditorLocalizations.localizationsDelegates,
///   supportedLocales: EditorLocalizations.supportedLocales,
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
/// be consistent with the languages listed in the EditorLocalizations.supportedLocales
/// property.
abstract class EditorLocalizations {
  EditorLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static EditorLocalizations of(BuildContext context) {
    return Localizations.of<EditorLocalizations>(context, EditorLocalizations)!;
  }

  static const LocalizationsDelegate<EditorLocalizations> delegate =
      _EditorLocalizationsDelegate();

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

  /// Title of the standalone note editor screen.
  ///
  /// In en, this message translates to:
  /// **'Note editor'**
  String get editorScreenTitle;

  /// Semantics label of the markdown editor area.
  ///
  /// In en, this message translates to:
  /// **'Note body'**
  String get editorBodyLabel;

  /// Note status: the editor has edits that were not saved yet.
  ///
  /// In en, this message translates to:
  /// **'Unsaved changes'**
  String get statusUnsaved;

  /// Title of the conflict banner above the editor.
  ///
  /// In en, this message translates to:
  /// **'This note was also changed on the server'**
  String get conflictBannerTitle;

  /// Body of the conflict banner.
  ///
  /// In en, this message translates to:
  /// **'Your edit overlaps with the server\'s version. Resolve the conflict to keep syncing this note.'**
  String get conflictBannerMessage;

  /// Button that opens the conflict screen.
  ///
  /// In en, this message translates to:
  /// **'Resolve'**
  String get conflictResolve;

  /// Empty state when the note does not exist.
  ///
  /// In en, this message translates to:
  /// **'Note not found'**
  String get noteNotFoundTitle;

  /// Empty state message when the note does not exist.
  ///
  /// In en, this message translates to:
  /// **'It may have been deleted or moved.'**
  String get noteNotFoundMessage;

  /// Error state title when the note stream fails.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t open this note'**
  String get noteLoadErrorTitle;

  /// Error state message when the note stream fails.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong while reading this note on this device.'**
  String get noteLoadErrorMessage;

  /// Snack bar when saving fails.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save. Your text is still here.'**
  String get saveFailed;

  /// Semantics label of the formatting toolbar.
  ///
  /// In en, this message translates to:
  /// **'Formatting'**
  String get toolbarLabel;

  /// Toolbar button: undo.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get toolUndo;

  /// Toolbar button: redo.
  ///
  /// In en, this message translates to:
  /// **'Redo'**
  String get toolRedo;

  /// Toolbar button: start a heading line.
  ///
  /// In en, this message translates to:
  /// **'Heading'**
  String get toolHeading;

  /// Toolbar button: bold.
  ///
  /// In en, this message translates to:
  /// **'Bold'**
  String get toolBold;

  /// Toolbar button: italic.
  ///
  /// In en, this message translates to:
  /// **'Italic'**
  String get toolItalic;

  /// Toolbar button: bulleted list item.
  ///
  /// In en, this message translates to:
  /// **'Bulleted list'**
  String get toolBulletList;

  /// Toolbar button: task line.
  ///
  /// In en, this message translates to:
  /// **'Checklist item'**
  String get toolChecklist;

  /// Toolbar button: start a [[wikilink]].
  ///
  /// In en, this message translates to:
  /// **'Insert wikilink'**
  String get toolWikilink;

  /// Toolbar button: start an @mention.
  ///
  /// In en, this message translates to:
  /// **'Mention a person or company'**
  String get toolMention;

  /// Toolbar button: start a #tag.
  ///
  /// In en, this message translates to:
  /// **'Tag'**
  String get toolTag;

  /// Toolbar button: hide the on-screen keyboard.
  ///
  /// In en, this message translates to:
  /// **'Hide keyboard'**
  String get toolHideKeyboard;

  /// Toolbar button: open the wikilink under the caret.
  ///
  /// In en, this message translates to:
  /// **'Open link'**
  String get toolOpenLink;

  /// Semantics label of an open task's checkbox.
  ///
  /// In en, this message translates to:
  /// **'Complete task: {task}'**
  String taskComplete({required String task});

  /// Semantics label of a done or cancelled task's checkbox.
  ///
  /// In en, this message translates to:
  /// **'Reopen task: {task}'**
  String taskReopen({required String task});

  /// Semantics label of a task checkbox whose task the core does not know yet.
  ///
  /// In en, this message translates to:
  /// **'Task line (save to enable)'**
  String get taskNotSynced;

  /// Semantics label of the autocomplete panel.
  ///
  /// In en, this message translates to:
  /// **'Suggestions'**
  String get suggestionsLabel;

  /// Autocomplete: nothing matches.
  ///
  /// In en, this message translates to:
  /// **'No matches'**
  String get suggestionsNone;

  /// Button that closes the autocomplete panel.
  ///
  /// In en, this message translates to:
  /// **'Dismiss suggestions'**
  String get suggestionsDismiss;

  /// Status bar: the editor edits markdown source.
  ///
  /// In en, this message translates to:
  /// **'Markdown'**
  String get statusBarMarkdown;

  /// Status bar: paragraph direction is automatic.
  ///
  /// In en, this message translates to:
  /// **'RTL · auto per paragraph'**
  String get statusBarRtl;

  /// Keyboard shortcut description: save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get shortcutSave;

  /// Completions heading for [[ links.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get suggestionsKindWikiLink;

  /// Completions heading for @ mentions.
  ///
  /// In en, this message translates to:
  /// **'People and companies'**
  String get suggestionsKindMention;

  /// Completions heading for # tags.
  ///
  /// In en, this message translates to:
  /// **'Tags'**
  String get suggestionsKindTag;

  /// Completions heading for [[Note#^ block references.
  ///
  /// In en, this message translates to:
  /// **'Blocks'**
  String get suggestionsKindBlock;

  /// Live preview toggle (turns preview off).
  ///
  /// In en, this message translates to:
  /// **'Show markdown'**
  String get showMarkdown;

  /// Live preview toggle (turns preview on).
  ///
  /// In en, this message translates to:
  /// **'Hide markdown'**
  String get hideMarkdown;
}

class _EditorLocalizationsDelegate
    extends LocalizationsDelegate<EditorLocalizations> {
  const _EditorLocalizationsDelegate();

  @override
  Future<EditorLocalizations> load(Locale locale) {
    return SynchronousFuture<EditorLocalizations>(
      lookupEditorLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_EditorLocalizationsDelegate old) => false;
}

EditorLocalizations lookupEditorLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return EditorLocalizationsAr();
    case 'en':
      return EditorLocalizationsEn();
  }

  throw FlutterError(
    'EditorLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
