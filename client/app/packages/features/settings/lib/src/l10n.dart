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

  /// The message of an integrity warning (1:1 on the core's `message_key`).
  String integrityMessage(IntegrityItem item) => switch (item.messageKey) {
    'integrity.temp_file_removed' => integrityTempFile,
    'integrity.uncommitted_changes' => integrityUncommitted,
    'integrity.id_assigned' => integrityIdAssigned,
    'integrity.out_of_band_edit' => integrityOutOfBand,
    'integrity.missing_file' => integrityMissingFile,
    'integrity.sidecar_repaired' => integritySidecar,
    'integrity.orphan_sidecar_removed' => integrityOrphanSidecar,
    'integrity.index_repaired' => integrityIndexRepaired,
    'integrity.interrupted_write_rolled_back' => integrityRolledBack,
    'integrity.op_result_recovered' => integrityRecovered,
    _ => integrityOther(kind: item.kind),
  };

  /// The localised message of a core failure.
  String failure(Object error) => switch (error) {
    CoreFailure(code: 'offline') => errorOffline,
    CoreFailure(:final code) => errorGeneric(code: code),
    _ => errorGeneric(code: 'internal'),
  };
}
