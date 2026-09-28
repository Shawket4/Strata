/// Strata ask feature (UI only; view-models come from the Rust core,
/// PLAN L15): [AskScreen] (chat over the vault with citations) and
/// [SearchScreen] (keyword / semantic / hybrid).
library;

import 'package:strata_ask/src/ask_screen.dart';
import 'package:strata_ask/src/search_screen.dart';

export 'src/ask_screen.dart' show AskScreen;
export 'src/l10n.dart' show AskLocalizationScope, AskLocalizations;
export 'src/search_screen.dart' show SearchScreen;
