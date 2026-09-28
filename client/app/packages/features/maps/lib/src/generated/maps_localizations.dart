import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'maps_localizations_ar.dart';
import 'maps_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of MapsLocalizations
/// returned by `MapsLocalizations.of(context)`.
///
/// Applications need to include `MapsLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/maps_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: MapsLocalizations.localizationsDelegates,
///   supportedLocales: MapsLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the MapsLocalizations.supportedLocales
/// property.
abstract class MapsLocalizations {
  MapsLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static MapsLocalizations of(BuildContext context) {
    return Localizations.of<MapsLocalizations>(context, MapsLocalizations)!;
  }

  static const LocalizationsDelegate<MapsLocalizations> delegate =
      _MapsLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('en'),
  ];

  /// Title of the global map.
  ///
  /// In en, this message translates to:
  /// **'Map'**
  String get mapTitle;

  /// Global map on a phone-sized window (only local mind maps are shown on compact).
  ///
  /// In en, this message translates to:
  /// **'The map needs a wider window'**
  String get mapCompactTitle;

  /// Body of the compact global map notice.
  ///
  /// In en, this message translates to:
  /// **'Open a note and use its local map, or make the window wider.'**
  String get mapCompactMessage;

  /// Empty global map.
  ///
  /// In en, this message translates to:
  /// **'Nothing to map yet'**
  String get mapEmptyTitle;

  /// Empty global map body.
  ///
  /// In en, this message translates to:
  /// **'Notes, people and their links appear here as you write.'**
  String get mapEmptyMessage;

  /// Header subtitle of the global map.
  ///
  /// In en, this message translates to:
  /// **'{nodes} nodes · {edges} edges · {clusters} clusters'**
  String mapCounts({
    required int nodes,
    required int edges,
    required int clusters,
  });

  /// Accessibility label of the global map canvas.
  ///
  /// In en, this message translates to:
  /// **'Global map with {nodes} nodes and {edges} links'**
  String mapSemantics({required int nodes, required int edges});

  /// Label of the lens selector.
  ///
  /// In en, this message translates to:
  /// **'Lens'**
  String get lensLabel;

  /// Lens: the note graph.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get lensNotes;

  /// Lens: people-centred graph.
  ///
  /// In en, this message translates to:
  /// **'People'**
  String get lensPeople;

  /// Lens: company-centred graph.
  ///
  /// In en, this message translates to:
  /// **'Companies'**
  String get lensCompanies;

  /// Placeholder of the map search field.
  ///
  /// In en, this message translates to:
  /// **'Search to focus…'**
  String get searchToFocus;

  /// Accessibility label of the map search field.
  ///
  /// In en, this message translates to:
  /// **'Search to focus'**
  String get searchToFocusLabel;

  /// Map search without results.
  ///
  /// In en, this message translates to:
  /// **'No matching notes'**
  String get searchNoResults;

  /// Status chip when a node is selected.
  ///
  /// In en, this message translates to:
  /// **'Focused on {title}'**
  String focusedOn({required String title});

  /// Clears the map selection.
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get clearFocus;

  /// Accessibility label of the zoom controls.
  ///
  /// In en, this message translates to:
  /// **'Zoom'**
  String get zoomGroup;

  /// Zoom in button.
  ///
  /// In en, this message translates to:
  /// **'Zoom in'**
  String get zoomIn;

  /// Zoom out button.
  ///
  /// In en, this message translates to:
  /// **'Zoom out'**
  String get zoomOut;

  /// Fit the whole graph in view.
  ///
  /// In en, this message translates to:
  /// **'Zoom to fit'**
  String get zoomToFit;

  /// Zoom indicator at far zoom.
  ///
  /// In en, this message translates to:
  /// **'{percent}% · labels: clusters'**
  String zoomLevelFar({required int percent});

  /// Zoom indicator at mid zoom.
  ///
  /// In en, this message translates to:
  /// **'{percent}% · labels: hubs'**
  String zoomLevelMid({required int percent});

  /// Zoom indicator at near zoom.
  ///
  /// In en, this message translates to:
  /// **'{percent}% · labels: all'**
  String zoomLevelNear({required int percent});

  /// Accessibility label of the minimap.
  ///
  /// In en, this message translates to:
  /// **'Minimap'**
  String get minimap;

  /// Filters button / panel title.
  ///
  /// In en, this message translates to:
  /// **'Filters'**
  String get filters;

  /// Filters drawer title (medium).
  ///
  /// In en, this message translates to:
  /// **'Map filters'**
  String get mapFilters;

  /// Resets the map filters.
  ///
  /// In en, this message translates to:
  /// **'Reset'**
  String get resetFilters;

  /// Closes the filters drawer.
  ///
  /// In en, this message translates to:
  /// **'Close filters'**
  String get closeFilters;

  /// Filter group: edge types.
  ///
  /// In en, this message translates to:
  /// **'Edge types'**
  String get edgeTypes;

  /// Filter: plain wikilinks.
  ///
  /// In en, this message translates to:
  /// **'body links'**
  String get edgeBodyLinks;

  /// Filter group: node kinds.
  ///
  /// In en, this message translates to:
  /// **'Node kinds'**
  String get nodeKinds;

  /// Similarity toggle title.
  ///
  /// In en, this message translates to:
  /// **'AI similarity'**
  String get similarityTitle;

  /// Similarity toggle help.
  ///
  /// In en, this message translates to:
  /// **'Faint links between look-alike notes. Never saved to your vault.'**
  String get similarityHelp;

  /// Filter group: focus one cluster.
  ///
  /// In en, this message translates to:
  /// **'Cluster focus'**
  String get clusterFocus;

  /// Cluster focus: none.
  ///
  /// In en, this message translates to:
  /// **'All clusters'**
  String get allClusters;

  /// Member count of a cluster.
  ///
  /// In en, this message translates to:
  /// **'{count}'**
  String clusterMembers({required int count});

  /// Degree of a node in the hover card.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 link} other{{count} links}}'**
  String hoverLinks({required int count});

  /// Opens the selected note.
  ///
  /// In en, this message translates to:
  /// **'Open note'**
  String get openNote;

  /// Opens the local mind map of the selected node.
  ///
  /// In en, this message translates to:
  /// **'Open local map'**
  String get openLocalMap;

  /// Subtitle of the local mind map.
  ///
  /// In en, this message translates to:
  /// **'Local map'**
  String get localMap;

  /// Depth control label.
  ///
  /// In en, this message translates to:
  /// **'Depth'**
  String get depth;

  /// Accessibility label of a depth option.
  ///
  /// In en, this message translates to:
  /// **'Depth {depth}'**
  String depthValue({required int depth});

  /// Recentre the mind map on a node.
  ///
  /// In en, this message translates to:
  /// **'Recentre on {title}'**
  String recentreOn({required String title});

  /// Aside action: recentre on the selected node.
  ///
  /// In en, this message translates to:
  /// **'Centre map on {title}'**
  String centreMapOn({required String title});

  /// Saves the mind map layout as a .canvas file.
  ///
  /// In en, this message translates to:
  /// **'Save layout'**
  String get saveLayout;

  /// Hint next to Save layout.
  ///
  /// In en, this message translates to:
  /// **'→ .canvas'**
  String get saveLayoutTarget;

  /// Tooltip of a disabled Save layout.
  ///
  /// In en, this message translates to:
  /// **'Saving layouts needs a connection to the server'**
  String get saveLayoutUnavailable;

  /// Mind map hint.
  ///
  /// In en, this message translates to:
  /// **'Drop a note on another to create a relation'**
  String get dragHint;

  /// Mind map hint, second part.
  ///
  /// In en, this message translates to:
  /// **'· drag to rearrange, then Save layout'**
  String get dragHintMore;

  /// Mind map status chip.
  ///
  /// In en, this message translates to:
  /// **'depth {depth} · {nodes} nodes · {edges} edges'**
  String mindMapCounts({
    required int depth,
    required int nodes,
    required int edges,
  });

  /// Accessibility label of the mind map canvas.
  ///
  /// In en, this message translates to:
  /// **'Local mind map for {title}'**
  String mindMapSemantics({required String title});

  /// Mind map of an unknown note.
  ///
  /// In en, this message translates to:
  /// **'This note isn\'t in the vault any more'**
  String get mindMapNotFound;

  /// Mind map of an unknown note, body.
  ///
  /// In en, this message translates to:
  /// **'It may have been deleted or moved on another device.'**
  String get mindMapNotFoundMessage;

  /// Edge sheet title (accessibility).
  ///
  /// In en, this message translates to:
  /// **'Selected relation'**
  String get selectedRelation;

  /// Edge sheet heading.
  ///
  /// In en, this message translates to:
  /// **'{from} → {to}'**
  String edgeFromTo({required String from, required String to});

  /// Accessibility label of an edge label.
  ///
  /// In en, this message translates to:
  /// **'{type}: {from} to {to}'**
  String edgeSemantics({
    required String type,
    required String from,
    required String to,
  });

  /// Heading of the AI reason.
  ///
  /// In en, this message translates to:
  /// **'Why AI suggested this'**
  String get whyAi;

  /// Shown until the core streams edge reasons.
  ///
  /// In en, this message translates to:
  /// **'The AI\'s reason for this link isn\'t shown here yet.'**
  String get reasonUnavailable;

  /// Change a relation's type.
  ///
  /// In en, this message translates to:
  /// **'Retype'**
  String get retype;

  /// Reject a relation.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get reject;

  /// Retype menu title.
  ///
  /// In en, this message translates to:
  /// **'Change relation type'**
  String get retypeTitle;

  /// Aside label (mind map, expanded).
  ///
  /// In en, this message translates to:
  /// **'Selected node'**
  String get selectedNode;

  /// Clears the selected node.
  ///
  /// In en, this message translates to:
  /// **'Clear selection'**
  String get clearSelection;

  /// Aside section: outgoing relations.
  ///
  /// In en, this message translates to:
  /// **'Relations'**
  String get relations;

  /// Aside section: incoming links.
  ///
  /// In en, this message translates to:
  /// **'Backlinks'**
  String get backlinks;

  /// Back button of the compact mind map.
  ///
  /// In en, this message translates to:
  /// **'Back to note'**
  String get backToNote;

  /// Mini graph section title.
  ///
  /// In en, this message translates to:
  /// **'Graph'**
  String get entityGraph;

  /// Opens the local mind map from the mini graph.
  ///
  /// In en, this message translates to:
  /// **'Open in map'**
  String get openInMap;

  /// Mini graph accessibility label.
  ///
  /// In en, this message translates to:
  /// **'Graph of {title}: {count, plural, =1{1 connected item} other{{count} connected items}}'**
  String miniGraphSemantics({required String title, required int count});

  /// Loading state label.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// Error state title.
  ///
  /// In en, this message translates to:
  /// **'This couldn\'t be loaded'**
  String get errorTitle;

  /// Error state body.
  ///
  /// In en, this message translates to:
  /// **'The app\'s local data returned an error ({code}).'**
  String errorMessage({required String code});

  /// Mind map label.
  ///
  /// In en, this message translates to:
  /// **'{kind}: {title}'**
  String nodeSemantics({required String kind, required String title});

  /// Mind map label.
  ///
  /// In en, this message translates to:
  /// **'Map / Local map'**
  String get mapBreadcrumb;

  /// Similarity switch subtitle while offline.
  ///
  /// In en, this message translates to:
  /// **'Similarity links need a connection to the server.'**
  String get similarityOffline;

  /// Switch that shows tags on the map.
  ///
  /// In en, this message translates to:
  /// **'Tags as nodes'**
  String get showTags;

  /// Node-kind chip with the core's count.
  ///
  /// In en, this message translates to:
  /// **'{kind} · {count}'**
  String kindWithCount({required String kind, required String count});

  /// Hover card: link count and the core's last-change label.
  ///
  /// In en, this message translates to:
  /// **'{links} · {updated}'**
  String hoverLinksUpdated({required String links, required String updated});

  /// Tooltip of Save layout.
  ///
  /// In en, this message translates to:
  /// **'Saves this map as a JSON Canvas file in your vault'**
  String get saveLayoutHint;

  /// Name field of the saved layout.
  ///
  /// In en, this message translates to:
  /// **'Map name'**
  String get layoutName;

  /// Snack bar after saving a layout.
  ///
  /// In en, this message translates to:
  /// **'Saved to {path}'**
  String layoutSaved({required String path});

  /// Cancels a dialog.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;
}

class _MapsLocalizationsDelegate
    extends LocalizationsDelegate<MapsLocalizations> {
  const _MapsLocalizationsDelegate();

  @override
  Future<MapsLocalizations> load(Locale locale) {
    return SynchronousFuture<MapsLocalizations>(
      lookupMapsLocalizations(locale),
    );
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['ar', 'en'].contains(locale.languageCode);

  @override
  bool shouldReload(_MapsLocalizationsDelegate old) => false;
}

MapsLocalizations lookupMapsLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return MapsLocalizationsAr();
    case 'en':
      return MapsLocalizationsEn();
  }

  throw FlutterError(
    'MapsLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
