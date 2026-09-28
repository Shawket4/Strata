import 'package:flutter/widgets.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/src/app.dart';
import 'package:strata/src/boot/file_picker.dart';
import 'package:strata_state/strata_state.dart'
    show BridgeCoreApi, coreApiProvider, filePickerProvider;
import 'package:strata_ui/strata_ui.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  registerStrataFontLicenses();
  runApp(
    ProviderScope(
      // Every core provider reads the Rust core through the bridge; the core
      // itself is loaded behind the splash (`coreStartupProvider`). Export
      // and import paths come from the native file dialogs.
      overrides: [
        coreApiProvider.overrideWithValue(const BridgeCoreApi()),
        filePickerProvider.overrideWithValue(const FileSelectorPicker()),
      ],
      child: const StrataApp(),
    ),
  );
}
