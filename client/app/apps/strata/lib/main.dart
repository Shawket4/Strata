import 'package:flutter/widgets.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/app.dart';
import 'package:strata_ui/strata_ui.dart';

void main() {
  registerStrataFontLicenses();
  runApp(const ProviderScope(child: StrataApp()));
}
