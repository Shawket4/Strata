import 'package:flutter/widgets.dart';
import 'package:strata_sync/src/generated/sync_localizations.dart';

export 'package:strata_sync/src/generated/sync_localizations.dart';

/// The sync feature's strings for the UI locale of [BuildContext].
///
/// Looked up synchronously from the ambient [Localizations] locale, so the
/// feature works inside any app that supports English and Arabic without
/// registering an extra delegate.
extension SyncL10nContext on BuildContext {
  /// Sync strings in the current UI language.
  SyncLocalizations get syncL10n =>
      lookupSyncLocalizations(Localizations.localeOf(this));
}
