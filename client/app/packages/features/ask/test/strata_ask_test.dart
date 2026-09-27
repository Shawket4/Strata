import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ask/strata_ask.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

Future<void> _pump(WidgetTester tester, Locale locale) {
  return tester.pumpWidget(
    MaterialApp(
      theme: StrataTheme.light(),
      locale: locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      home: const Scaffold(body: AskScreen()),
    ),
  );
}

void main() {
  testWidgets('renders its localized title as a header in English', (
    tester,
  ) async {
    await _pump(tester, StrataLocales.english);
    await tester.pumpAndSettle();
    expect(find.text('Ask'), findsOneWidget);
    expect(find.text('This screen is being built.'), findsOneWidget);
    final title = tester.getSemantics(find.text('Ask')).getSemanticsData();
    expect(title.flagsCollection.isHeader, isTrue);
    final icon = tester.widget<Icon>(
      find.descendant(of: find.byType(AskScreen), matching: find.byType(Icon)),
    );
    expect(icon.icon, AskScreen.icon);
  });

  testWidgets('renders right-to-left in Arabic', (tester) async {
    await _pump(tester, StrataLocales.arabic);
    await tester.pumpAndSettle();
    expect(find.text('اسأل'), findsOneWidget);
    expect(
      Directionality.of(tester.element(find.byType(AskScreen))),
      TextDirection.rtl,
    );
  });
}
