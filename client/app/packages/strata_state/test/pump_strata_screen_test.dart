import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

/// A minimal screen in the shape feature screens take: render the home
/// view-model, forward a capture intent.
class _ProbeScreen extends ConsumerWidget {
  const new();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final home = ref.watch(homeProvider);
    return Scaffold(
      body: switch (home) {
        AsyncData(:final value) => Column(
          children: [
            for (final note in value.recentNotes) Text(note.title),
            TextButton(
              onPressed: () => ref
                  .read(coreApiProvider)
                  .capture(text: 'Idea: loyalty tier for customers'),
              child: Text(context.l10n.navHome),
            ),
          ],
        ),
        AsyncError() => const Text('error'),
        _ => const Text('loading'),
      },
    );
  }
}

void main() {
  testWidgets('wires the fake core, renders its values, records intents', (
    tester,
  ) async {
    final fake = FakeCoreApi()..home.add(StrataFixtures.homeView);
    final returned = await pumpStrataScreen(
      tester,
      const _ProbeScreen(),
      fake: fake,
    );
    expect(returned, same(fake));
    expect(find.text('Pricing experiments'), findsOneWidget);
    expect(find.text('Call 2026-09-12 — Acme'), findsOneWidget);
    expect(find.text('تجارب التسعير — ملخص'), findsOneWidget);

    await tester.tap(find.text('Home'));
    expect(fake.calls, const [
      CoreCall('watchHome'),
      CoreCall('capture', {'text': 'Idea: loyalty tier for customers'}),
    ]);
  });

  testWidgets('creates a fake when none is given; values arrive later', (
    tester,
  ) async {
    final fake = await pumpStrataScreen(tester, const _ProbeScreen());
    expect(find.text('loading'), findsOneWidget);
    fake.home.add(StrataFixtures.homeView);
    await tester.pump();
    expect(find.text('Pricing experiments'), findsOneWidget);
  });

  testWidgets('defaults: compact, light, English, text scale 1', (
    tester,
  ) async {
    await pumpStrataScreen(tester, const _ProbeScreen());
    final context = tester.element(find.byType(_ProbeScreen));
    expect(MediaQuery.sizeOf(context), StrataTestSizes.compact);
    expect(SizeClass.of(context), SizeClass.compact);
    expect(Theme.of(context).brightness, Brightness.light);
    expect(Directionality.of(context), TextDirection.ltr);
    expect(MediaQuery.textScalerOf(context), TextScaler.noScaling);
  });

  testWidgets('size class, dark theme, Arabic and text scale 2', (
    tester,
  ) async {
    await pumpStrataScreen(
      tester,
      const _ProbeScreen(),
      sizeClass: SizeClass.expanded,
      theme: Brightness.dark,
      locale: StrataLocales.arabic,
      textScale: 2,
    );
    final context = tester.element(find.byType(_ProbeScreen));
    expect(MediaQuery.sizeOf(context), StrataTestSizes.expanded);
    expect(Theme.of(context).brightness, Brightness.dark);
    expect(Directionality.of(context), TextDirection.rtl);
    expect(context.l10n.navHome, 'الرئيسية');
    expect(MediaQuery.textScalerOf(context), const TextScaler.linear(2));
  });

  testWidgets('an explicit size wins', (tester) async {
    await pumpStrataScreen(
      tester,
      const _ProbeScreen(),
      size: StrataTestSizes.medium,
    );
    final context = tester.element(find.byType(_ProbeScreen));
    expect(SizeClass.of(context), SizeClass.medium);
  });

  testWidgets('extra overrides apply on top of the fake core', (tester) async {
    await pumpStrataScreen(
      tester,
      const _ProbeScreen(),
      overrides: [
        homeProvider.overrideWithValue(AsyncData(StrataFixtures.homeView)),
      ],
    );
    expect(find.text('Pricing experiments'), findsOneWidget);
  });
}
