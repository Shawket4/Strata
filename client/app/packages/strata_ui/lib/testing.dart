/// Test support for packages that render `strata_ui` widgets: loads the
/// bundled fonts so widget and golden tests measure and paint real glyphs.
///
/// Import only from tests (e.g. in `flutter_test_config.dart`).
library;

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:strata_ui/src/tokens/typography.dart';

/// Window sizes every screen is tested at (PLAN §16.5).
abstract final class StrataTestSizes {
  /// Compact phone: 390 × 844.
  static const Size compact = Size(390, 844);

  /// Medium tablet / narrow window: 1024 × 768.
  static const Size medium = Size(1024, 768);

  /// Expanded laptop: 1440 × 900.
  static const Size expanded = Size(1440, 900);

  /// Large desktop: 1920 × 1080.
  static const Size large = Size(1920, 1080);

  /// All four sizes with their names.
  static const Map<String, Size> all = {
    'compact': compact,
    'medium': medium,
    'expanded': expanded,
    'large': large,
  };
}

final Set<String> _loaded = {};

/// Loads every font in the asset bundle's `FontManifest.json` (the bundled
/// Cairo, IBM Plex Mono, Quicksand and Material Icons).
///
/// Families are registered under the manifest name and, for fonts owned by
/// `strata_ui`, also under `packages/strata_ui/<family>` so styles with
/// `package: 'strata_ui'` resolve in `strata_ui`'s own tests too.
Future<void> loadStrataFonts({AssetBundle? bundle}) async {
  final assets = bundle ?? rootBundle;
  final manifest =
      jsonDecode(await assets.loadString('FontManifest.json')) as List<Object?>;
  const packagePrefix = 'packages/${StrataFonts.package}/';
  const ownFamilies = {
    StrataFonts.cairo,
    StrataFonts.plexMono,
    StrataFonts.quicksand,
  };
  for (final entry in manifest.cast<Map<String, Object?>>()) {
    final family = entry['family']! as String;
    final fonts = (entry['fonts']! as List<Object?>)
        .cast<Map<String, Object?>>()
        .map((f) => f['asset']! as String)
        .toList();
    final names = {family};
    if (ownFamilies.contains(family)) names.add('$packagePrefix$family');
    for (final name in names) {
      if (!_loaded.add(name)) continue;
      final loader = FontLoader(name);
      for (final asset in fonts) {
        loader.addFont(assets.load(asset));
      }
      await loader.load();
    }
  }
}
