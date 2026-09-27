import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_ui/strata_ui.dart';

Future<void> _pump(WidgetTester tester, Locale locale) {
  return tester.pumpWidget(
    MaterialApp(
      theme: StrataTheme.light(),
      locale: locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      home: const Scaffold(body: SettingsScreen()),
    ),
  );
}

void main() {
  testWidgets('renders its localized title as a header in English', (
    tester,
  ) async {
    await _pump(tester, StrataLocales.english);
    await tester.pumpAndSettle();
    expect(find.text('Settings'), findsOneWidget);
    expect(find.text('This screen is being built.'), findsOneWidget);
    final title = tester.getSemantics(find.text('Settings')).getSemanticsData();
    expect(title.flagsCollection.isHeader, isTrue);
    final icon = tester.widget<Icon>(
      find.descendant(
        of: find.byType(SettingsScreen),
        matching: find.byType(Icon),
      ),
    );
    expect(icon.icon, SettingsScreen.icon);
  });

  testWidgets('renders right-to-left in Arabic', (tester) async {
    await _pump(tester, StrataLocales.arabic);
    await tester.pumpAndSettle();
    expect(find.text('الإعدادات'), findsOneWidget);
    expect(
      Directionality.of(tester.element(find.byType(SettingsScreen))),
      TextDirection.rtl,
    );
  });
}
