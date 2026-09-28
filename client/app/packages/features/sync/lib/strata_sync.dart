/// Strata sync feature (PLAN §11 screen 12b): the sync pill, the sync status
/// as a sheet (compact), drawer (medium) or popover (expanded), the `/sync`
/// page and conflict resolution. UI only; view-models come from the Rust
/// core (L15).
library;

export 'src/conflict_screen.dart';
export 'src/l10n.dart';
export 'src/labels.dart' show SyncLabels;
export 'src/sync_panel.dart';
export 'src/sync_pill.dart';
export 'src/sync_surfaces.dart';
