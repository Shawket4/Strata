/// Strata maps feature (UI only; view-models come from the Rust core,
/// PLAN L15): the global map ([GlobalMapScreen], one `CustomPainter`, D3 = a),
/// the local mind map ([MindMapScreen], widget canvas, D4 = b) and the
/// embeddable [MiniGraph].
library;

import 'package:strata_maps/src/global_map/global_map_screen.dart';
import 'package:strata_maps/src/mind_map/mind_map_screen.dart';
import 'package:strata_maps/src/mini_graph.dart';

export 'src/common/l10n.dart' show MapsLocalizations;
export 'src/global_map/global_map_screen.dart' show GlobalMapScreen;
export 'src/graph/graph_camera.dart'
    show GraphCamera, GraphViewController, ZoomBand;
export 'src/graph/graph_kinds.dart';
export 'src/graph/graph_painter.dart'
    show GraphPaintOptions, GraphPainter, GraphPalette, GraphRenderCache;
export 'src/graph/graph_scene.dart' show GraphScene;
export 'src/mind_map/mind_map_screen.dart' show MindMapScreen;
export 'src/mind_map/node_aside.dart' show relationTypeOfWire;
export 'src/mini_graph.dart' show MiniGraph;

/// The route shell's name for the global map entry widget.
typedef MapScreen = GlobalMapScreen;
