import 'dart:async';

import 'package:strata_state/testing.dart';

/// The shared alchemist configuration (CI goldens with the bundled fonts).
Future<void> testExecutable(FutureOr<void> Function() testMain) =>
    strataTestExecutable(testMain);
