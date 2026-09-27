/// Strata inbox feature (UI only; view-models come from the Rust core,
/// PLAN L15): captures with AI proposals, entity link-or-create, custody,
/// duplicate-flagged items and other suggestions.
library;

export 'src/capture_card.dart';
export 'src/inbox_screen.dart';
export 'src/l10n.dart'
    show InboxL10nContext, InboxL10nScope, InboxLocalizations;
export 'src/proposals.dart';
