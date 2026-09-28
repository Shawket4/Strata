import 'package:flutter/widgets.dart';
import 'package:strata_accounts/src/generated/accounts_localizations.dart';
import 'package:strata_state/strata_state.dart';

export 'package:strata_accounts/src/generated/accounts_localizations.dart';

/// The accounts feature's strings for the UI locale of [BuildContext]
/// (looked up synchronously; no extra delegate to register).
extension AccountsL10nContext on BuildContext {
  /// Accounts strings in the current UI language.
  AccountsLocalizations get accountsL10n =>
      lookupAccountsLocalizations(Localizations.localeOf(this));
}

/// 1:1 renderings of account codes from the core.
extension AccountsLabels on AccountsLocalizations {
  /// The localised message of a failure thrown by the core
  /// (`CoreFailure.code`, PLAN §12.1 "typed errors").
  String failure(Object error) => switch (error) {
    CoreFailure(code: 'invalid_credentials') => errorInvalidCredentials,
    CoreFailure(code: 'offline') => errorOffline,
    CoreFailure(code: 'account_pending') => errorAccountPending,
    CoreFailure(code: 'account_rejected') => errorAccountRejected,
    CoreFailure(code: 'account_disabled') => errorAccountDisabled,
    CoreFailure(code: 'account_deletion_pending') => errorAccountDeletion,
    CoreFailure(code: 'session_expired') => errorSessionExpired,
    CoreFailure(code: 'rate_limited') => errorRateLimited,
    CoreFailure(code: 'not_available') => errorNotAvailable,
    CoreFailure(
      code: 'invalid_input',
      field: 'server_url',
      reason: 'insecure_http',
    ) =>
      errorInsecureServer,
    CoreFailure(code: 'invalid_input', :final field) => errorInvalidInput(
      field: field ?? '',
    ),
    CoreFailure(code: 'server', :final status) => errorServer(
      status: '${status ?? ''}',
    ),
    CoreFailure(:final code) => errorGeneric(code: code),
    _ => errorGeneric(code: 'internal'),
  };

  /// The name of an account role (`admin` | `member`).
  String role(String role) => switch (role) {
    'admin' => roleAdmin,
    _ => roleMember,
  };
}
