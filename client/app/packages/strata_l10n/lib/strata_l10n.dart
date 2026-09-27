/// Strata UI strings (English and Arabic), generated from the ARB files in
/// `lib/l10n` by `flutter gen-l10n`.
///
/// UI direction follows the UI language (PLAN §11): Arabic is laid out
/// right-to-left through [GlobalWidgetsLocalizations], which is part of
/// [StrataLocalizations.localizationsDelegates].
library;

import 'package:flutter/widgets.dart';
import 'package:strata_l10n/src/generated/strata_localizations.dart';

export 'src/generated/strata_localizations.dart';

/// Convenience access to [StrataLocalizations] from a [BuildContext].
extension StrataLocalizationsContext on BuildContext {
  /// The [StrataLocalizations] for the closest [Localizations] ancestor.
  StrataLocalizations get l10n => StrataLocalizations.of(this);
}

/// Locales the Strata UI ships with.
abstract final class StrataLocales {
  /// English (template language).
  static const Locale english = Locale('en');

  /// Arabic (right-to-left UI).
  static const Locale arabic = Locale('ar');
}
