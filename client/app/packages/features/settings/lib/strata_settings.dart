/// Strata settings feature (PLAN §11 screen 13): account, devices,
/// reminders, AI status, integrity, export/import, sync, Admin → Users and
/// about. UI only; view-models come from the Rust core (L15).
library;

export 'src/l10n.dart';
export 'src/sections.dart' show RemindersSection, SectionContent;
export 'src/settings_screen.dart';
