import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/testing.dart';

/// Opens the picker and shows what it returned.
class _Launcher extends StatefulWidget {
  const new();

  @override
  State<_Launcher> createState() => _LauncherState();
}

class _LauncherState extends State<_Launcher> {
  String _picked = 'none';

  @override
  Widget build(BuildContext context) => Column(
    children: [
      TextButton(
        onPressed: () async {
          final zone = await showTimeZonePicker(context);
          setState(() => _picked = zone ?? 'cancelled');
        },
        child: const Text('open'),
      ),
      Text('picked $_picked'),
    ],
  );
}

void main() {
  group('time zone picker matrix', () {
    for (final v in variants()) {
      testWidgets('lists the core zones $v', (tester) async {
        final l10n = lookupSettingsLocalizations(v.locale);
        final fake = await pumpVariant(
          tester,
          v,
          const TimeZonePickerDialog(),
          scaffold: true,
        );
        expect(fake.calls, [
          const CoreCall('timezones', {'query': ''}),
        ]);
        // Every row: the core's name, region, IANA ID and offset; the
        // account's zone is marked.
        final list = find.descendant(
          of: find.byType(ListView),
          matching: find.byType(Scrollable),
        );
        for (final zone in StrataFixtures.timezones) {
          await tester.scrollUntilVisible(
            find.text(zone.id),
            60,
            scrollable: list,
          );
          expect(find.text(zone.name), findsOneWidget);
          expect(find.text(zone.offsetLabel), findsWidgets);
        }
        await tester.scrollUntilVisible(
          find.text(l10n.timezoneCurrent),
          -60,
          scrollable: list,
        );
        expect(find.text(l10n.timezoneCurrent), findsOneWidget);
        expect(
          directionOf(tester, find.text(l10n.timezoneCurrent)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('time zone picker', () {
    final v = variants().first;

    testWidgets('the core matches the search; nothing found says so', (
      tester,
    ) async {
      final fake = FakeCoreApi()..timezonesAnswer.returns(const []);
      await pumpVariant(
        tester,
        v,
        const TimeZonePickerDialog(),
        fake: fake,
        scaffold: true,
      );
      await tester.enterText(find.byType(TextField), 'atlantis');
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('timezones', {'query': 'atlantis'}),
      );
      expect(find.text('No time zone matches “atlantis”'), findsOneWidget);
    });

    testWidgets('picking a zone returns its IANA ID; cancel returns none', (
      tester,
    ) async {
      await pumpVariant(tester, v, const _Launcher(), scaffold: true);
      await tester.tap(find.text('open'));
      await settle(tester);
      await tester.tap(find.text('Riyadh'));
      await settle(tester);
      expect(find.text('picked Asia/Riyadh'), findsOneWidget);
      await tester.tap(find.text('open'));
      await settle(tester);
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(find.text('picked cancelled'), findsOneWidget);
    });
  });
}
