import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart' show AccountSheet;
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart' show SyncStatusContent;
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

FakeCoreApi _fake([SettingsView view = StrataFixtures.settingsView]) =>
    FakeCoreApi()
      ..session.add(StrataFixtures.sessionActive)
      ..settings.add(view)
      ..syncStatus.add(StrataFixtures.syncStatusView);

/// A stand-in for Admin → Users in the wide content pane.
class _AdminPane extends StatelessWidget {
  const new();

  @override
  Widget build(BuildContext context) => const Center(child: Text('ADMIN PANE'));
}

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v, {
  SettingsSection? section,
  FakeCoreApi? fake,
  ValueChanged<SettingsSection?>? onSelect,
}) => pumpVariant(
  tester,
  v,
  Scaffold(
    body: SettingsScreen(
      section: section,
      onSelectSection: onSelect ?? (_) {},
      adminPane: const _AdminPane(),
      onOpenConflict: (_) {},
    ),
  ),
  fake: fake ?? _fake(),
);

void main() {
  group('settings matrix', () {
    for (final v in matrix()) {
      testWidgets('home $v', (tester) async {
        final l10n = lookupSettingsLocalizations(v.locale);
        final fake = await _pump(tester, v);
        expect(fake.calls, contains(const CoreCall('watchSettings')));
        if (v.sizeClass == SizeClass.compact) {
          // The list of sections; nothing selected.
          expect(find.text('Shawket'), findsOneWidget);
          expect(find.text(l10n.sectionReminders), findsOneWidget);
          await tester.scrollUntilVisible(
            find.text(l10n.signOut),
            200,
            scrollable: find.byType(Scrollable).first,
          );
          expect(find.text(l10n.sectionAdmin), findsOneWidget);
          expect(find.bySemanticsLabel(l10n.settingsNav), findsNothing);
        } else {
          // Navigation (224) + the account section.
          expect(find.bySemanticsLabel(l10n.settingsNav), findsOneWidget);
          expect(find.text(l10n.sectionAccount), findsNWidgets(2));
          expect(find.text(l10n.changePasswordTitle), findsWidgets);
          expect(find.text('Africa/Cairo'), findsOneWidget);
        }
        expect(
          directionOf(tester, find.byType(SettingsScreen)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      for (final (section, check)
          in <(SettingsSection, void Function(SettingsLocalizations))>[
            (
              SettingsSection.devices,
              (l10n) {
                expect(find.text('shawket-laptop'), findsOneWidget);
                expect(find.text(l10n.remindersOnDevice), findsOneWidget);
              },
            ),
            (
              SettingsSection.reminders,
              (l10n) {
                expect(
                  find.text(l10n.remindersPermissionGranted),
                  findsOneWidget,
                );
                expect(find.text(l10n.deliveryOs), findsOneWidget);
                expect(
                  find.text(l10n.scheduledCount(count: 4)),
                  findsOneWidget,
                );
                expect(find.text('09:00'), findsOneWidget);
              },
            ),
            (
              SettingsSection.ai,
              (l10n) {
                expect(find.text(l10n.unavailableNotYet), findsOneWidget);
                expect(find.text(l10n.aiBody), findsOneWidget);
              },
            ),
            (
              SettingsSection.integrity,
              (l10n) {
                expect(find.text(l10n.integrityBody), findsOneWidget);
              },
            ),
            (
              SettingsSection.data,
              (l10n) {
                expect(find.text(l10n.exportVault), findsOneWidget);
                expect(find.text(l10n.importFiles), findsOneWidget);
              },
            ),
            (
              SettingsSection.sync,
              (l10n) {
                expect(find.byType(SyncStatusContent), findsOneWidget);
              },
            ),
            (
              SettingsSection.about,
              (l10n) {
                expect(find.byType(StrataWordmark), findsOneWidget);
                expect(find.text(l10n.licences), findsOneWidget);
              },
            ),
            (SettingsSection.admin, (l10n) {}),
          ]) {
        testWidgets('${section.name} $v', (tester) async {
          final l10n = lookupSettingsLocalizations(v.locale);
          await _pump(tester, v, section: section);
          check(l10n);
          if (v.sizeClass == SizeClass.compact) {
            expect(find.byType(AppBar), findsOneWidget);
            expect(find.text(sectionTitle(l10n, section)), findsWidgets);
          } else {
            expect(find.bySemanticsLabel(l10n.settingsNav), findsOneWidget);
          }
          if (section == SettingsSection.admin) {
            expect(
              find.text('ADMIN PANE'),
              v.sizeClass == SizeClass.compact ? findsNothing : findsOneWidget,
            );
          }
          expectNoErrors(tester);
          await expectAccessible(tester, contrast: v.textScale == 1);
        });
      }

      testWidgets('reminders off on this device $v', (tester) async {
        final l10n = lookupSettingsLocalizations(v.locale);
        await _pump(
          tester,
          v,
          section: SettingsSection.reminders,
          fake: _fake(SettingsFixtures.denied),
        );
        expect(find.text(l10n.remindersOffPermission), findsNWidgets(2));
        expect(find.text(l10n.deliveryWhileRunning), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('settings', () {
    final v = matrix().first;
    final expanded = matrix().firstWhere((v) => v.sizeName == 'expanded');

    testWidgets('loading until the core emits', (tester) async {
      await _pump(tester, v, fake: FakeCoreApi());
      expect(find.bySemanticsLabel('Loading settings'), findsOneWidget);
    });

    testWidgets('error', (tester) async {
      final fake = FakeCoreApi();
      await _pump(tester, v, fake: fake);
      fake.settings.addError(
        const CoreFailure(code: 'storage', messageKey: 'error.storage'),
      );
      await settle(tester);
      expect(find.text('Settings could not be loaded'), findsOneWidget);
      expect(find.text('Something went wrong (storage).'), findsOneWidget);
    });

    testWidgets('a member sees no Admin group', (tester) async {
      await _pump(tester, v, fake: _fake(SettingsFixtures.member));
      expect(find.text('Users'), findsNothing);
      expect(find.text('العربية'), findsOneWidget);
    });

    testWidgets('compact list opens sections', (tester) async {
      final selected = <SettingsSection?>[];
      await _pump(tester, v, onSelect: selected.add);
      await tapVisible(tester, find.text('Reminders'));
      await tester.scrollUntilVisible(
        find.text('Users'),
        200,
        scrollable: find.byType(Scrollable).first,
      );
      await tapVisible(tester, find.text('Users'));
      expect(selected, [SettingsSection.reminders, SettingsSection.admin]);
    });

    testWidgets('compact section back returns to the list', (tester) async {
      final selected = <SettingsSection?>[];
      await _pump(
        tester,
        v,
        section: SettingsSection.about,
        onSelect: selected.add,
      );
      await tester.tap(find.byType(BackButton));
      expect(selected, [null]);
    });

    testWidgets('wide navigation selects sections', (tester) async {
      final selected = <SettingsSection?>[];
      await _pump(tester, expanded, onSelect: selected.add);
      await tapVisible(tester, find.text('Sync'));
      await tapVisible(tester, find.text('About'));
      expect(selected, [SettingsSection.sync, SettingsSection.about]);
    });

    testWidgets('the reminders switch forwards the device setting', (
      tester,
    ) async {
      final fake = await _pump(tester, v, section: SettingsSection.reminders);
      await tapVisible(tester, find.text('Reminders on this device'));
      expect(
        fake.calls.last,
        const CoreCall('setRemindersEnabled', {'enabled': false}),
      );
    });

    testWidgets('the account header opens the account sheet', (tester) async {
      await _pump(tester, v);
      await tapVisible(tester, find.text('Shawket'));
      expect(find.byType(AccountSheet), findsOneWidget);
    });

    testWidgets('sign out from the list', (tester) async {
      final fake = await _pump(tester, v);
      await tester.scrollUntilVisible(
        find.text('Sign out'),
        200,
        scrollable: find.byType(Scrollable).first,
      );
      await tapVisible(tester, find.text('Sign out'));
      expect(fake.calls.last, const CoreCall('signOut', {'force': false}));
    });

    testWidgets('sign out from the account section', (tester) async {
      final fake = await _pump(tester, expanded);
      await tapVisible(tester, find.widgetWithText(OutlinedButton, 'Sign out'));
      expect(fake.calls.last, const CoreCall('signOut', {'force': false}));
    });

    testWidgets('licences', (tester) async {
      await _pump(tester, expanded, section: SettingsSection.about);
      await tapVisible(tester, find.text('Licences'));
      expect(find.byType(LicensePage), findsOneWidget);
    });

    testWidgets('actions the core lacks are disabled', (tester) async {
      await _pump(tester, expanded, section: SettingsSection.data);
      final export = tester.widget<ButtonStyleButton>(
        find.ancestor(
          of: find.text('Export vault (.zip)'),
          matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
        ),
      );
      expect(export.onPressed, isNull);
    });

    testWidgets('admin card opens Admin → Users without a pane', (
      tester,
    ) async {
      final selected = <SettingsSection?>[];
      await pumpVariant(
        tester,
        v,
        Scaffold(
          body: SettingsScreen(
            section: SettingsSection.admin,
            onSelectSection: selected.add,
          ),
        ),
        fake: _fake(),
      );
      await tapVisible(tester, find.text('Open Admin → Users'));
      expect(selected, [SettingsSection.admin]);
    });

    test('section names round-trip', () {
      for (final section in SettingsSection.values) {
        expect(SettingsSection.tryParse(section.name), section);
      }
      expect(SettingsSection.tryParse('nope'), isNull);
      expect(SettingsSection.tryParse(null), isNull);
      final l10n = lookupSettingsLocalizations(const Locale('en'));
      expect(l10n.availability(Availability.offline), 'Needs a connection');
      expect(l10n.failure(Exception()), 'Something went wrong (internal).');
      expect(
        l10n.failure(const CoreFailure(code: 'offline', messageKey: 'e')),
        "You're offline.",
      );
      expect(SettingsScreen.icon, Icons.settings_outlined);
    });
  });
}
