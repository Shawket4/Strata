/// The Strata Rust client core (PLAN §12), bridged to Dart with
/// flutter_rust_bridge v2. Plumbing only (L15): the generated bindings, the
/// library loader and the app-support directory the core stores its
/// databases in. Every decision is made in Rust.
library;

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:path_provider/path_provider.dart';
import 'package:strata_bridge/src/generated/frb_generated.dart';

export 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

export 'src/generated/api/app.dart';
export 'src/generated/api/intents.dart';
export 'src/generated/api/reminders.dart';
export 'src/generated/api/views.dart';
export 'src/generated/frb_generated.dart' show StrataCore;
export 'src/generated/view/model.dart';

/// Loads the native core library (bundled by cargokit on every platform;
/// tests pass [library] explicitly).
Future<void> loadStrataCore({ExternalLibrary? library}) =>
    StrataCore.init(externalLibrary: library);

/// The platform's app-support directory: where the core keeps the device
/// registry and one database per account (the value of
/// `CoreConfig.appDataDir`).
Future<String> strataAppDataDirectory() async =>
    (await getApplicationSupportDirectory()).path;
