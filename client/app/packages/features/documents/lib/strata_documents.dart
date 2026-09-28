/// Strata documents feature (UI only; view-models come from the Rust core,
/// PLAN L15): document pages ([DocumentScreen]) and place pages
/// ([PlaceScreen]) with custody history and "Record a move".
library;

import 'package:strata_documents/src/document/document_screen.dart';
import 'package:strata_documents/src/place/place_screen.dart';

export 'src/common/custody.dart'
    show
        CustodyList,
        CustodyTile,
        DocumentBriefTile,
        DocumentStatusPill,
        HolderLine,
        PlaceBreadcrumb;
export 'src/common/l10n.dart'
    show DocumentsLocalizationScope, DocumentsLocalizations;
export 'src/common/labels.dart';
export 'src/common/mentions.dart';
export 'src/common/record_move.dart'
    show MoveEvent, RecordMoveForm, openRecordMove, showRecordMove;
export 'src/common/user_notes.dart';
export 'src/common/widgets.dart';
export 'src/document/document_screen.dart' show DocumentPage, DocumentScreen;
export 'src/document/documents_list_screen.dart' show DocumentsScreen;
export 'src/place/place_screen.dart' show PlacePage, PlaceScreen;
