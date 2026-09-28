import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata/strata.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;
import 'package:strata_ui/testing.dart';

import 'fakes.dart';

/// The pieces of a booted app.
class Booted {
  /// Creates the record.
  new(this.fake, this.platform, this.bootstrap, this.container);

  /// The core.
  final FakeCoreApi fake;

  /// The notification plugin seam.
  final FakeNotificationPlatform platform;

  /// The native bootstrap stand-in.
  final FakeBootstrap bootstrap;

  /// The root provider container.
  final ProviderContainer container;

  /// The router.
  GoRouter get router => container.read(appRouterProvider);
}

/// Boots the real app ([StrataApp] in a root container) with [fake] as the
/// core, starting in [session] (also the first value of the session
/// stream), at [size] / [brightness] / [locale] / [textScale].
Future<Booted> boot(
  WidgetTester tester, {
  SessionState session = StrataFixtures.sessionActive,
  FakeCoreApi? fake,
  FakeNotificationPlatform? platform,
  FakeBootstrap? bootstrap,
  Size size = StrataTestSizes.compact,
  Brightness brightness = Brightness.light,
  Locale locale = StrataLocales.english,
  double textScale = 1,
  bool emitSession = true,
  bool seed = true,
  bool stubInit = true,
}) async {
  tester.view
    ..physicalSize = size
    ..devicePixelRatio = 1;
  tester.platformDispatcher
    ..platformBrightnessTestValue = brightness
    ..textScaleFactorTestValue = textScale;
  addTearDown(tester.view.reset);
  addTearDown(tester.platformDispatcher.clearAllTestValues);
  final core = fake ?? FakeCoreApi();
  addTearDown(core.dispose);
  if (stubInit) core.initCoreAnswer.returns(session);
  if (emitSession) core.session.add(session);
  if (seed) {
    core
      ..syncStatus.add(StrataFixtures.syncStatusView)
      ..home.add(StrataFixtures.homeView)
      ..settings.add(StrataFixtures.settingsView);
  }
  final plugin = platform ?? FakeNotificationPlatform();
  final loader = bootstrap ?? FakeBootstrap();
  final container = ProviderContainer.test(
    overrides: [
      coreApiProvider.overrideWithValue(core),
      coreBootstrapProvider.overrideWithValue(loader),
      notificationPlatformProvider.overrideWithValue(plugin),
    ],
  );
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: StrataApp(locale: locale),
    ),
  );
  await settle(tester);
  return Booted(core, plugin, loader, container);
}

/// Navigates to [location] and lets the pages build.
Future<void> go(WidgetTester tester, Booted app, String location) async {
  app.router.go(location);
  await settle(tester);
}

/// The size class of a window of [size].
SizeClass sizeClassOf(Size size) => SizeClass.fromWidth(size.width);
