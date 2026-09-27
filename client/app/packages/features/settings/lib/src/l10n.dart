import 'package:flutter/widgets.dart';
import 'package:strata_settings/src/generated/settings_localizations.dart';
import 'package:strata_state/strata_state.dart';

export 'package:strata_settings/src/generated/settings_localizations.dart';

/// The settings feature's strings for the UI locale of [BuildContext]
/// (looked up synchronously; no extra delegate to register).
extension SettingsL10nContext on BuildContext {
  /// Settings strings in the current UI language.
  SettingsLocalizations get settingsL10n =>
      lookupSettingsLocalizations(Localizations.localeOf(this));
}

/// 1:1 renderings of settings codes from the core.
extension SettingsLabels on SettingsLocalizations {
  /// The name of a role.
  String roleName(String role) => switch (role) {
    'admin' => roleAdmin,
    _ => roleMember,
  };

  /// The name of a UI language code (`en` | `ar`).
  String languageName(String code) => switch (code) {
    'ar' => languageAr,
    _ => languageEn,
  };

  /// The short state of an online-only or not-yet-built feature.
  String availability(Availability availability) => switch (availability) {
    Availability.available => available,
    Availability.offline => unavailableOffline,
    Availability.notYetAvailable => unavailableNotYet,
    Availability.notAllowed => unavailableNotAllowed,
  };

  /// The localised message of a core failure.
  String failure(Object error) => switch (error) {
    CoreFailure(code: 'offline') => errorOffline,
    CoreFailure(:final code) => errorGeneric(code: code),
    _ => errorGeneric(code: 'internal'),
  };
}
