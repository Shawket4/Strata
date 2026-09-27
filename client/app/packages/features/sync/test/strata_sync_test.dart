import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart';

Future<void> _pump(WidgetTester tester, Locale locale) {
  return tester.pumpWidget(
    MaterialApp(
      theme: StrataTheme.light(),
      locale: locale,
      supportedLocales: StrataLocalizations.supportedLocales,
      localizationsDelegates: StrataLocalizations.localizationsDelegates,
      home: const Scaffold(body: SyncScreen()),
    ),
  );
}

void main() {
  testWidgets('renders its localized title as a header in English', (
    tester,
  ) async {
    await _pump(tester, StrataLocales.english);
    await tester.pumpAndSettle();
    expect(find.text('Sync & conflicts'), findsOneWidget);
    expect(find.text('This screen is being built.'), findsOneWidget);
    final title = tester
        .getSemantics(find.text('Sync & conflicts'))
        .getSemanticsData();
    expect(title.flagsCollection.isHeader, isTrue);
    final icon = tester.widget<Icon>(
      find.descendant(of: find.byType(SyncScreen), matching: find.byType(Icon)),
    );
    expect(icon.icon, SyncScreen.icon);
  });

  testWidgets('renders right-to-left in Arabic', (tester) async {
    await _pump(tester, StrataLocales.arabic);
    await tester.pumpAndSettle();
    expect(find.text('المزامنة والتعارضات'), findsOneWidget);
    expect(
      Directionality.of(tester.element(find.byType(SyncScreen))),
      TextDirection.rtl,
    );
  });
}
