import 'package:flutter/material.dart' show Icons;
import 'package:flutter/widgets.dart';

import 'package:strata_admin/src/generated/admin_localizations.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

export 'package:strata_admin/src/generated/admin_localizations.dart';

/// The admin feature's strings for the UI locale of [BuildContext] (looked
/// up synchronously; no extra delegate to register).
extension AdminL10nContext on BuildContext {
  /// Admin strings in the current UI language.
  AdminLocalizations get adminL10n =>
      lookupAdminLocalizations(Localizations.localeOf(this));
}

/// 1:1 renderings of the account codes in [AdminUserItem].
extension AdminLabels on AdminLocalizations {
  /// The name of a role.
  String role(String role) => switch (role) {
    'admin' => roleAdmin,
    _ => roleMember,
  };

  /// The localised message of a core failure.
  String failure(Object error) => switch (error) {
    CoreFailure(code: 'offline') => errorOffline,
    CoreFailure(:final code) => errorGeneric(code: code),
    _ => errorGeneric(code: 'internal'),
  };
}

/// The tone of an account status (`pending`, `active`, `disabled`,
/// `rejected`, `deletion_pending`).
StatusTone statusTone(String status) => switch (status) {
  'active' => StatusTone.success,
  'pending' => StatusTone.info,
  'disabled' || 'rejected' => StatusTone.neutral,
  'deletion_pending' => StatusTone.danger,
  _ => StatusTone.neutral,
};

/// The icon of an account status.
IconData statusIcon(String status) => switch (status) {
  'active' => Icons.check_circle_outline,
  'pending' => Icons.schedule,
  'disabled' => Icons.block,
  'rejected' => Icons.cancel_outlined,
  'deletion_pending' => Icons.auto_delete_outlined,
  _ => Icons.help_outline,
};
