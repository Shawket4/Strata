import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_accounts/strata_accounts.dart' show AccountSheet;
import 'package:strata_settings/strata_settings.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_sync/strata_sync.dart' show SyncStatusContent;
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';

FakeCoreApi _fake([SettingsView? view]) => FakeCoreApi()
  ..session.add(StrataFixtures.sessionActive)
  ..settings.add(view ?? SettingsFixtures.full)
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
  SettingsScreen(
    section: section,
    onSelectSection: onSelect ?? (_) {},
    adminPane: const _AdminPane(),
    onOpenConflict: (_) {},
  ),
  fake: fake ?? _fake(),
  scaffold: true,
);

/// The intents after opening the screen (`refreshSettings` and the
/// streams).
List<CoreCall> _intents(FakeCoreApi fake) => [
  for (final call in fake.calls)
    if (!call.method.startsWith('watch') && call.method != 'refreshSettings')
      call,
];

void main() {
  group('settings matrix', () {
    for (final v in variants()) {
      testWidgets('home $v', (tester) async {
        final l10n = lookupSettingsLocalizations(v.locale);
        final fake = await _pump(tester, v);
        expect(fake.calls, contains(const CoreCall('watchSettings')));
        expect(fake.calls.first, const CoreCall('refreshSettings'));
        if (v.sizeClass == SizeClass.compact) {
          // The list of sections; nothing selected.
          expect(find.text('Shawket'), findsOneWidget);
          expect(find.text('S'), findsOneWidget);
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
          expect(find.text(l10n.changePasswordTitle), findsOneWidget);
          expect(find.text('Africa/Cairo'), findsOneWidget);
          expect(find.text(l10n.languageEn), findsOneWidget);
          expect(find.byTooltip(l10n.editTimezoneTitle), findsOneWidget);
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
                expect(find.text('Updated 14:32'), findsOneWidget);
                expect(find.text('shawket-laptop'), findsOneWidget);
                expect(find.text('Pixel 9'), findsOneWidget);
                expect(find.text(l10n.thisDeviceBadge), findsOneWidget);
                expect(
                  find.text(
                    l10n.deviceDetail(lastSeen: '2h ago', signedIn: '12 Mar'),
                  ),
                  findsOneWidget,
                );
                expect(
                  find.byTooltip(l10n.revokeDevice(name: 'shawket-laptop')),
                  findsNothing,
                );
                expect(
                  find.byTooltip(l10n.revokeDevice(name: 'Pixel 9')),
                  findsOneWidget,
                );
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
                expect(
                  find.text(l10n.snoozeMinutes(count: 10)),
                  findsOneWidget,
                );
              },
            ),
            (
              SettingsSection.ai,
              (l10n) {
                expect(
                  find.text('AI paused — daily budget reached'),
                  findsOneWidget,
                );
                expect(
                  find.text(l10n.aiOn(provider: 'Anthropic')),
                  findsOneWidget,
                );
                expect(find.text(l10n.aiQueue(count: 3)), findsOneWidget);
                expect(find.text(l10n.aiFailedJobs(count: 2)), findsOneWidget);
                expect(find.text(l10n.aiRetryFailed), findsOneWidget);
                expect(find.text(r'$2.00 of $2.00 today'), findsOneWidget);
              },
            ),
            (
              SettingsSection.integrity,
              (l10n) {
                expect(find.text(l10n.integrityOutOfBand), findsOneWidget);
                expect(
                  find.text('notes/sales/Pricing experiments.md'),
                  findsOneWidget,
                );
                expect(
                  find.text(l10n.integrityOther(kind: 'disk_full')),
                  findsOneWidget,
                );
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

      testWidgets('server parts offline $v', (tester) async {
        final l10n = lookupSettingsLocalizations(v.locale);
        await _pump(
          tester,
          v,
          section: SettingsSection.devices,
          fake: _fake(SettingsFixtures.denied),
        );
        expect(find.text(l10n.unavailableOffline), findsOneWidget);
        expect(find.text(l10n.offlineBody), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('settings', () {
    final v = variants().first;
    final expanded = variants().firstWhere((v) => v.sizeName == 'expanded');

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

    testWidgets('display name and time zone are saved by the core', (
      tester,
    ) async {
      final fake = await _pump(tester, expanded);
      await tapVisible(tester, find.byTooltip('Display name'));
      await tester.enterText(find.byType(TextField).last, 'Shawket I.');
      await tester.tap(find.text('Save'));
      await settle(tester);
      // The zone comes from the core's list: search, then pick.
      await tapVisible(tester, find.byTooltip('Time zone'));
      await tester.enterText(find.byType(TextField).last, 'riy');
      await settle(tester);
      await tester.tap(find.text('Riyadh'));
      await settle(tester);
      expect(_intents(fake), [
        const CoreCall('setDisplayName', {'name': 'Shawket I.'}),
        const CoreCall('timezones', {'query': ''}),
        const CoreCall('timezones', {'query': 'riy'}),
        const CoreCall('setTimezone', {'iana': 'Asia/Riyadh'}),
      ]);
      expect(find.text('Saved'), findsWidgets);
    });

    testWidgets('the language picker sets the UI language', (tester) async {
      final fake = await _pump(tester, expanded);
      await tapVisible(tester, find.text('English'));
      await tester.tap(find.text('العربية').last);
      await settle(tester);
      expect(_intents(fake), [
        const CoreCall('setUiLanguage', {'code': 'ar'}),
      ]);
    });

    testWidgets('change password sends both passwords', (tester) async {
      final fake = await _pump(tester, expanded);
      await tester.enterText(
        find.widgetWithText(TextField, 'Current password'),
        'old-tide',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'New password'),
        'new-sand-4821',
      );
      await tapVisible(tester, find.text('Change password'));
      expect(_intents(fake), [
        const CoreCall('changePassword', {
          'current': 'old-tide',
          'new_': 'new-sand-4821',
        }),
      ]);
      expect(find.text('Password changed'), findsOneWidget);
    });

    testWidgets('a refused password change shows the failure', (tester) async {
      final fake = _fake()
        ..changePasswordAnswer.throws(
          const CoreFailure(code: 'wrong_password', messageKey: 'e'),
        );
      await _pump(tester, expanded, fake: fake);
      await tapVisible(tester, find.text('Change password'));
      expect(
        find.text('Something went wrong (wrong_password).'),
        findsOneWidget,
      );
    });

    testWidgets('AI: Retry queues the failed jobs and says how many', (
      tester,
    ) async {
      final fake = _fake()..retryFailedJobsAnswer.returns(2);
      await _pump(tester, expanded, fake: fake, section: SettingsSection.ai);
      await tapVisible(tester, find.text('Retry'));
      expect(_intents(fake), [const CoreCall('retryFailedJobs')]);
      expect(find.text('2 jobs queued again'), findsOneWidget);

      fake.retryFailedJobsAnswer.throws(
        const CoreFailure(code: 'offline', messageKey: 'e'),
      );
      await tapVisible(tester, find.text('Retry'));
      expect(
        find.text(lookupSettingsLocalizations(const Locale('en')).errorOffline),
        findsOneWidget,
      );
    });

    testWidgets('AI: no failed jobs, no Retry', (tester) async {
      await _pump(
        tester,
        expanded,
        fake: _fake(SettingsFixtures.withFailedJobs(0)),
        section: SettingsSection.ai,
      );
      expect(find.text('Retry'), findsNothing);
      expect(find.text('2 AI jobs failed'), findsNothing);
    });

    testWidgets('devices: rename, sign out, reminders, refresh', (
      tester,
    ) async {
      final fake = await _pump(
        tester,
        expanded,
        section: SettingsSection.devices,
      );
      await tapVisible(tester, find.byTooltip('Rename Pixel 9'));
      await tester.enterText(find.byType(TextField).last, 'Pixel 9 Pro');
      await tester.tap(find.text('Save'));
      await settle(tester);
      await tapVisible(tester, find.byTooltip('Sign out MacBook Pro'));
      expect(find.text('Sign MacBook Pro out?'), findsOneWidget);
      await tester.tap(find.text('Revoke'));
      await settle(tester);
      await tapVisible(tester, find.text('Reminders on MacBook Pro'));
      await tapVisible(tester, find.byTooltip('Refresh'));
      expect(fake.calls.where((c) => !c.method.startsWith('watch')), [
        const CoreCall('refreshSettings'),
        const CoreCall('renameDevice', {
          'id': 'd-pixel',
          'name': 'Pixel 9 Pro',
        }),
        const CoreCall('revokeDevice', {'id': 'd-mac'}),
        const CoreCall('setDeviceReminders', {'id': 'd-mac', 'enabled': true}),
        const CoreCall('refreshSettings'),
      ]);
    });

    testWidgets('reminders: default time, snooze, quiet hours', (tester) async {
      final fake = await _pump(
        tester,
        expanded,
        section: SettingsSection.reminders,
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Time (HH:MM)'),
        '08:30',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      await tapVisible(tester, find.text('10 minutes'));
      await tester.tap(find.text('30 minutes').last);
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, 'Until (HH:MM)'),
        '06:30',
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await settle(tester);
      await tapVisible(tester, find.text('Quiet hours'));
      expect(_intents(fake), [
        const CoreCall('setDefaultReminderTime', {'time': '08:30'}),
        const CoreCall('setSnoozeMinutes', {'minutes': 30}),
        const CoreCall('setQuietHours', {
          'enabled': true,
          'from': '22:00',
          'until': '06:30',
        }),
        const CoreCall('setQuietHours', {
          'enabled': false,
          'from': '22:00',
          'until': '06:30',
        }),
      ]);
    });

    testWidgets('the reminders switch forwards the device setting', (
      tester,
    ) async {
      final fake = await _pump(tester, v, section: SettingsSection.reminders);
      await tapVisible(tester, find.text('Reminders on this device'));
      expect(_intents(fake), [
        const CoreCall('setRemindersEnabled', {'enabled': false}),
      ]);
    });

    testWidgets('export and import paths come from the file dialogs', (
      tester,
    ) async {
      final fake = _fake()
        ..importVaultAnswer.returns(
          const ImportSummary(imported: 12, skipped: 1),
        );
      fake.files.saveFileAnswer.returns('/tmp/vault.zip');
      fake.files.openFileAnswer.returns('/tmp/in.zip');
      await _pump(tester, expanded, section: SettingsSection.data, fake: fake);
      await tapVisible(tester, find.text('Export vault (.zip)'));
      await settle(tester);
      expect(
        find.text('Exported ${StrataFixtures.exportSummary.label}'),
        findsOneWidget,
      );
      await tapVisible(tester, find.text('Import markdown…'));
      await settle(tester);
      expect(find.text('12 imported · 1 skipped'), findsOneWidget);
      expect(_intents(fake), [
        const CoreCall('saveFile', {
          'suggestedName': 'strata-vault.zip',
          'type': PickedFileType.zip,
        }),
        const CoreCall('exportVault', {'path': '/tmp/vault.zip'}),
        const CoreCall('openFile', {'type': PickedFileType.zip}),
        const CoreCall('importVault', {'path': '/tmp/in.zip'}),
      ]);
    });

    testWidgets('a cancelled file dialog asks the core nothing', (
      tester,
    ) async {
      final fake = _fake();
      fake.files.saveFileAnswer.returns(null);
      fake.files.openFileAnswer.returns(null);
      await _pump(tester, expanded, section: SettingsSection.data, fake: fake);
      await tapVisible(tester, find.text('Export vault (.zip)'));
      await tapVisible(tester, find.text('Import markdown…'));
      await settle(tester);
      expect(
        [for (final c in _intents(fake)) c.method],
        ['saveFile', 'openFile'],
      );
    });

    testWidgets('export and import wait for the server', (tester) async {
      await _pump(
        tester,
        expanded,
        section: SettingsSection.data,
        fake: _fake(SettingsFixtures.denied),
      );
      final export = tester.widget<ButtonStyleButton>(
        find.ancestor(
          of: find.text('Export vault (.zip)'),
          matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
        ),
      );
      expect(export.onPressed, isNull);
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

    testWidgets('admin card opens Manage users without a pane', (tester) async {
      final selected = <SettingsSection?>[];
      await pumpVariant(
        tester,
        v,
        SettingsScreen(
          section: SettingsSection.admin,
          onSelectSection: selected.add,
        ),
        fake: _fake(),
        scaffold: true,
      );
      await tapVisible(tester, find.text('Manage users'));
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
      expect(platformIcon('android'), Icons.smartphone_outlined);
      expect(platformIcon('linux'), Icons.laptop_outlined);
      expect(platformIcon('web'), Icons.devices_other_outlined);
    });
  });
}
