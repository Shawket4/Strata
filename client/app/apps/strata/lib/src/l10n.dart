import 'package:flutter/widgets.dart';
import 'package:strata/src/generated/app_localizations.dart';

export 'package:strata/src/generated/app_localizations.dart';

/// The app shell's own strings for the UI locale of [BuildContext].
extension AppL10nContext on BuildContext {
  /// App strings in the current UI language.
  AppLocalizations get appL10n =>
      lookupAppLocalizations(Localizations.localeOf(this));
}

/// The app strings for [locale], falling back to English (used outside the
/// widget tree, e.g. for notification action labels).
AppLocalizations appL10nFor(Locale locale) => lookupAppLocalizations(
  AppLocalizations.supportedLocales.any(
        (l) => l.languageCode == locale.languageCode,
      )
      ? Locale(locale.languageCode)
      : const Locale('en'),
);
