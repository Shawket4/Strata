import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/property_dialog.dart';
import 'package:strata_state/testing.dart';

/// Opens the dialog for a phone property with two numbers and shows what
/// it returned.
class _Launcher extends StatefulWidget {
  const new();

  @override
  State<_Launcher> createState() => _LauncherState();
}

class _LauncherState extends State<_Launcher> {
  String _result = 'none';

  @override
  Widget build(BuildContext context) => Column(
    children: [
      TextButton(
        onPressed: () async {
          final edit = await showPropertyDialog(
            context,
            title: 'Edit phone',
            key: 'phone',
            values: const ['+20 100', '+20 101'],
          );
          setState(
            () => _result = edit == null
                ? 'cancelled'
                : '${edit.key}=${edit.values.join('|')}',
          );
        },
        child: const Text('open'),
      ),
      Text('result $_result'),
    ],
  );
}

void main() {
  group('property dialog matrix', () {
    for (final v in variants()) {
      testWidgets('every value has a field $v', (tester) async {
        final l10n = lookupDirectoryLocalizations(v.locale);
        await pumpVariant(
          tester,
          v,
          const DirectoryLocalizationScope(
            child: PropertyDialog(
              title: 'phone',
              initialKey: 'phone',
              initialValues: ['+20 100', '+20 101'],
            ),
          ),
          scaffold: true,
        );
        final values = find.widgetWithText(TextField, l10n.propertyValue);
        expect(values, findsNWidgets(2));
        expect(
          [
            for (final f in tester.widgetList<TextField>(values))
              f.controller!.text,
          ],
          ['+20 100', '+20 101'],
        );
        expect(find.byTooltip(l10n.removeValue), findsNWidgets(2));
        expect(find.text(l10n.addValue), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  testWidgets('returns the key and every value; cancel returns none', (
    tester,
  ) async {
    await pumpVariant(
      tester,
      variants().first,
      const _Launcher(),
      scaffold: true,
    );
    await tester.tap(find.text('open'));
    await settle(tester);
    await tester.tap(find.byTooltip('Remove value').first);
    await settle(tester);
    // One value left: nothing to remove.
    expect(find.byTooltip('Remove value'), findsNothing);
    await tester.tap(find.text('Add value'));
    await settle(tester);
    await tester.enterText(
      find.widgetWithText(TextField, 'Value').last,
      '+20 102',
    );
    await tester.tap(find.text('Save'));
    await settle(tester);
    expect(find.text('result phone=+20 101|+20 102'), findsOneWidget);
    await tester.tap(find.text('open'));
    await settle(tester);
    await tester.tap(find.text('Cancel'));
    await settle(tester);
    expect(find.text('result cancelled'), findsOneWidget);
  });
}
