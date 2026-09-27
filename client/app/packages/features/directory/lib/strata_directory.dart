/// Strata directory feature (UI only; view-models come from the Rust core,
/// PLAN L15): the [DirectoryScreen] (People / Companies / Documents /
/// Places) and the person / company [EntityScreen] (which also renders
/// document and place pages for their IDs).
library;

import 'package:strata_directory/src/directory/directory_screen.dart';
import 'package:strata_directory/src/entity/entity_screen.dart';

export 'package:strata_documents/strata_documents.dart'
    show DocumentScreen, EntityLinks, OpenNoteAt, PlaceScreen;

export 'src/common/l10n.dart'
    show DirectoryLocalizationScope, DirectoryLocalizations;
export 'src/directory/directory_screen.dart' show DirectoryRow, DirectoryScreen;
export 'src/directory/new_entity.dart';
export 'src/directory/suggestions.dart';
export 'src/entity/entity_screen.dart'
    show EntityDetail, EntityPage, EntityScreen, entityKindOf;
export 'src/entity/sections.dart';

/// The route shell's former name for the directory entry widget.
typedef PeopleScreen = DirectoryScreen;
