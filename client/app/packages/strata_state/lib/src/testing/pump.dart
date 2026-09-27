import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/src/core_api.dart';
import 'package:strata_state/src/providers.dart';
import 'package:strata_state/src/testing/fake_core_api.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// The app frame a screen is tested in: a root provider container whose
/// `coreApiProvider` is [api] (plus [overrides]), and a [MaterialApp] with
/// the Strata themes, [locale] (English and Arabic delegates) and
/// [textScale].
///
/// The container is created once, when the frame is first built, and
/// disposed with it. Use the frame directly where a widget (not a tester) is
/// needed, e.g. alchemist golden scenarios; widget tests use
/// [pumpStrataScreen].
class StrataTestFrame extends StatefulWidget {
  /// Creates the frame around [child].
  const new({
    required this.api,
    required this.child,
    super.key,
    this.brightness = Brightness.light,
    this.locale = StrataLocales.english,
    this.textScale = 1,
    this.overrides = const [],
  });

  /// The core the providers read (usually a [FakeCoreApi]).
  final CoreApi api;

  /// The screen under test.
  final Widget child;

  /// Light or dark Strata theme.
  final Brightness brightness;

  /// UI locale (direction follows it).
  final Locale locale;

  /// System text scale (1.0 and 2.0 in the test matrix).
  final double textScale;

  /// Extra provider overrides.
  final List<Override> overrides;

  @override
  State<StrataTestFrame> createState() => _StrataTestFrameState();
}

class _StrataTestFrameState extends State<StrataTestFrame> {
  late final ProviderContainer _container = ProviderContainer(
    overrides: [
      coreApiProvider.overrideWithValue(widget.api),
      ...widget.overrides,
    ],
  );

  @override
  void dispose() {
    _container.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textScale = widget.textScale;
    return UncontrolledProviderScope(
      container: _container,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: StrataTheme.light(),
        darkTheme: StrataTheme.dark(),
        themeMode: widget.brightness == Brightness.dark
            ? ThemeMode.dark
            : ThemeMode.light,
        locale: widget.locale,
        supportedLocales: StrataLocalizations.supportedLocales,
        localizationsDelegates: StrataLocalizations.localizationsDelegates,
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: TextScaler.linear(textScale)),
          child: child!,
        ),
        home: widget.child,
      ),
    );
  }
}

/// Pumps [screen] inside a [StrataTestFrame] at a window [size] (default
/// compact 390 × 844; or pass a [sizeClass] to use its test size from
/// `StrataTestSizes`) and returns the [FakeCoreApi] it reads (a new one
/// unless [fake] is given; disposed at the end of the test).
///
/// The first frame is pumped, then one more so values already added to the
/// fake's streams reach the widgets.
Future<FakeCoreApi> pumpStrataScreen(
  WidgetTester tester,
  Widget screen, {
  FakeCoreApi? fake,
  Size? size,
  SizeClass? sizeClass,
  Brightness theme = Brightness.light,
  Locale locale = StrataLocales.english,
  double textScale = 1,
  List<Override> overrides = const [],
}) async {
  assert(
    size == null || sizeClass == null,
    'Pass either size or sizeClass, not both.',
  );
  final api = fake ?? FakeCoreApi();
  addTearDown(api.dispose);
  tester.view
    ..physicalSize =
        size ??
        switch (sizeClass) {
          null || SizeClass.compact => StrataTestSizes.compact,
          SizeClass.medium => StrataTestSizes.medium,
          SizeClass.expanded => StrataTestSizes.expanded,
        }
    ..devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    StrataTestFrame(
      api: api,
      brightness: theme,
      locale: locale,
      textScale: textScale,
      overrides: overrides,
      child: screen,
    ),
  );
  await tester.pump();
  return api;
}
