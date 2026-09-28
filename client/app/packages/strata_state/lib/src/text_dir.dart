import 'package:flutter/widgets.dart';
import 'package:strata_bridge/strata_bridge.dart' show TextDir;

/// The [TextDirection] of a content direction the core computed (`*_dir`
/// fields, Unicode P2 first-strong rule): `null` for neutral text, which
/// then follows the ambient direction. A 1:1 adapter, no detection in Dart
/// (L15).
TextDirection? textDirectionOf(TextDir dir) => switch (dir) {
  TextDir.ltr => TextDirection.ltr,
  TextDir.rtl => TextDirection.rtl,
  TextDir.neutral => null,
};
