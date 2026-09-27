import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_directory/src/generated/directory_localizations.dart';
import 'package:strata_directory/strata_directory.dart';
import 'package:strata_state/strata_state.dart' hide EntityScreen;
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import 'helpers/matrix.dart';

/// Live resize across the breakpoints re-evaluates the pane structure
/// (PLAN §16.5).
void main() {
  testWidgets('directory follows the window across size classes', (
    tester,
  ) async {
    final fake = FakeCoreApi();
    fake.directory[(DirectoryTab.people, '')].add(StrataFixtures.directoryView);
    await pumpVariant(tester, variants().first, const DirectoryScreen(), fake);
    final l10n = lookupDirectoryLocalizations(const Locale('en'));
    final table = find.bySemanticsLabel(l10n.tableCaption(tab: l10n.tabPeople));
    expect(table, findsNothing);
    expect(find.text(l10n.selectSomething), findsNothing);

    tester.view.physicalSize = StrataTestSizes.medium;
    await tester.pump();
    expect(find.text(l10n.selectSomething), findsOneWidget);
    expect(table, findsNothing);

    tester.view.physicalSize = StrataTestSizes.expanded;
    await tester.pump();
    expect(table, findsOneWidget);

    tester.view.physicalSize = StrataTestSizes.compact;
    await tester.pump();
    expect(table, findsNothing);
    expect(find.text('Ahmed Samir'), findsOneWidget);
  });
}
