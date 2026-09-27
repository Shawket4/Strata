import 'package:flutter/widgets.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/app.dart';
import 'package:strata_state/strata_state.dart'
    show BridgeCoreApi, coreApiProvider;
import 'package:strata_ui/strata_ui.dart';

void main() {
  registerStrataFontLicenses();
  runApp(
    ProviderScope(
      // Every core provider reads the Rust core through the bridge.
      overrides: [coreApiProvider.overrideWithValue(const BridgeCoreApi())],
      child: const StrataApp(),
    ),
  );
}
