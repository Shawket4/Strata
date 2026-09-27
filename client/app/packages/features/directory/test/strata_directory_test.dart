import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

Future<void> _pump(WidgetTester tester, Locale locale) {
  return tester.pumpWidget(
    MaterialApp(
      theme: StrataTheme.light(),
      locale: locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      home: const Scaffold(body: DirectoryScreen()),
    ),
  );
}

void main() {
  testWidgets('renders its localized title as a header in English', (
    tester,
  ) async {
    await _pump(tester, StrataLocales.english);
    await tester.pumpAndSettle();
    expect(find.text('Directory'), findsOneWidget);
    expect(find.text('This screen is being built.'), findsOneWidget);
    final title = tester
        .getSemantics(find.text('Directory'))
        .getSemanticsData();
    expect(title.flagsCollection.isHeader, isTrue);
    final icon = tester.widget<Icon>(
      find.descendant(
        of: find.byType(DirectoryScreen),
        matching: find.byType(Icon),
      ),
    );
    expect(icon.icon, DirectoryScreen.icon);
  });

  testWidgets('renders right-to-left in Arabic', (tester) async {
    await _pump(tester, StrataLocales.arabic);
    await tester.pumpAndSettle();
    expect(find.text('الدليل'), findsOneWidget);
    expect(
      Directionality.of(tester.element(find.byType(DirectoryScreen))),
      TextDirection.rtl,
    );
  });
}
