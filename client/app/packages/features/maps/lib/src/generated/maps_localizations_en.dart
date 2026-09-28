// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'maps_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class MapsLocalizationsEn extends MapsLocalizations {
  MapsLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get mapTitle => 'Map';

  @override
  String get mapCompactTitle => 'The map needs a wider window';

  @override
  String get mapCompactMessage =>
      'Open a note and use its local map, or make the window wider.';

  @override
  String get mapEmptyTitle => 'Nothing to map yet';

  @override
  String get mapEmptyMessage =>
      'Notes, people and their links appear here as you write.';

  @override
  String mapCounts({
    required int nodes,
    required int edges,
    required int clusters,
  }) {
    return '$nodes nodes · $edges edges · $clusters clusters';
  }

  @override
  String mapSemantics({required int nodes, required int edges}) {
    return 'Global map with $nodes nodes and $edges links';
  }

  @override
  String get lensLabel => 'Lens';

  @override
  String get lensNotes => 'Notes';

  @override
  String get lensPeople => 'People';

  @override
  String get lensCompanies => 'Companies';

  @override
  String get searchToFocus => 'Search to focus…';

  @override
  String get searchToFocusLabel => 'Search to focus';

  @override
  String get searchNoResults => 'No matching notes';

  @override
  String focusedOn({required String title}) {
    return 'Focused on $title';
  }

  @override
  String get clearFocus => 'Clear';

  @override
  String get zoomGroup => 'Zoom';

  @override
  String get zoomIn => 'Zoom in';

  @override
  String get zoomOut => 'Zoom out';

  @override
  String get zoomToFit => 'Zoom to fit';

  @override
  String zoomLevelFar({required int percent}) {
    return '$percent% · labels: clusters';
  }

  @override
  String zoomLevelMid({required int percent}) {
    return '$percent% · labels: hubs';
  }

  @override
  String zoomLevelNear({required int percent}) {
    return '$percent% · labels: all';
  }

  @override
  String get minimap => 'Minimap';

  @override
  String get filters => 'Filters';

  @override
  String get mapFilters => 'Map filters';

  @override
  String get resetFilters => 'Reset';

  @override
  String get closeFilters => 'Close filters';

  @override
  String get edgeTypes => 'Edge types';

  @override
  String get edgeBodyLinks => 'body links';

  @override
  String get nodeKinds => 'Node kinds';

  @override
  String get similarityTitle => 'AI similarity';

  @override
  String get similarityHelp =>
      'Faint links between look-alike notes. Never saved to your vault.';

  @override
  String get clusterFocus => 'Cluster focus';

  @override
  String get allClusters => 'All clusters';

  @override
  String clusterMembers({required int count}) {
    return '$count';
  }

  @override
  String hoverLinks({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count links',
      one: '1 link',
    );
    return '$_temp0';
  }

  @override
  String get openNote => 'Open note';

  @override
  String get openLocalMap => 'Open local map';

  @override
  String get localMap => 'Local map';

  @override
  String get depth => 'Depth';

  @override
  String depthValue({required int depth}) {
    return 'Depth $depth';
  }

  @override
  String recentreOn({required String title}) {
    return 'Recentre on $title';
  }

  @override
  String centreMapOn({required String title}) {
    return 'Centre map on $title';
  }

  @override
  String get saveLayout => 'Save layout';

  @override
  String get saveLayoutTarget => 'as .canvas';

  @override
  String get saveLayoutUnavailable =>
      'Saving layouts needs a connection to the server';

  @override
  String get dragHint => 'Drop a note on another to create a relation';

  @override
  String get dragHintMore => '· drag to rearrange, then Save layout';

  @override
  String mindMapCounts({
    required int depth,
    required int nodes,
    required int edges,
  }) {
    return 'depth $depth · $nodes nodes · $edges edges';
  }

  @override
  String mindMapSemantics({required String title}) {
    return 'Local mind map for $title';
  }

  @override
  String get mindMapNotFound => 'This note isn\'t in the vault any more';

  @override
  String get mindMapNotFoundMessage =>
      'It may have been deleted or moved on another device.';

  @override
  String get selectedRelation => 'Selected relation';

  @override
  String edgeFromTo({required String from, required String to}) {
    return '$from to $to';
  }

  @override
  String edgeSemantics({
    required String type,
    required String from,
    required String to,
  }) {
    return '$type: $from to $to';
  }

  @override
  String get whyAi => 'Why AI suggested this';

  @override
  String get reasonUnavailable =>
      'The AI\'s reason for this link isn\'t shown here yet.';

  @override
  String get retype => 'Retype';

  @override
  String get reject => 'Reject';

  @override
  String get retypeTitle => 'Change relation type';

  @override
  String get selectedNode => 'Selected node';

  @override
  String get clearSelection => 'Clear selection';

  @override
  String get relations => 'Relations';

  @override
  String get backlinks => 'Backlinks';

  @override
  String get backToNote => 'Back to note';

  @override
  String get entityGraph => 'Graph';

  @override
  String get openInMap => 'Open in map';

  @override
  String miniGraphSemantics({required String title, required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count connected items',
      one: '1 connected item',
    );
    return 'Graph of $title: $_temp0';
  }

  @override
  String get loading => 'Loading…';

  @override
  String get errorTitle => 'This couldn\'t be loaded';

  @override
  String errorMessage({required String code}) {
    return 'The app\'s local data returned an error ($code).';
  }

  @override
  String nodeSemantics({required String kind, required String title}) {
    return '$kind: $title';
  }

  @override
  String get mapBreadcrumb => 'Map / Local map';

  @override
  String get similarityOffline =>
      'Similarity links need a connection to the server.';

  @override
  String get showTags => 'Tags as nodes';

  @override
  String kindWithCount({required String kind, required String count}) {
    return '$kind · $count';
  }

  @override
  String hoverLinksUpdated({required String links, required String updated}) {
    return '$links · $updated';
  }

  @override
  String get saveLayoutHint =>
      'Saves this map as a JSON Canvas file in your vault';

  @override
  String get layoutName => 'Map name';

  @override
  String layoutSaved({required String path}) {
    return 'Saved to $path';
  }

  @override
  String get cancel => 'Cancel';
}
