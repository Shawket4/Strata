import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';

Future<FakeCoreApi> _pump(
  WidgetTester tester,
  Variant v,
  AdminUsersView view, {
  bool embedded = false,
}) async {
  final fake = FakeCoreApi()..loadAdminUsersAnswer.returns(view);
  await pumpVariant(
    tester,
    v,
    AdminUsersScreen(embedded: embedded),
    fake: fake,
    scaffold: true,
  );
  return fake;
}

/// The calls after the first list read.
List<CoreCall> _intents(FakeCoreApi fake) => fake.calls.skip(1).toList();

void main() {
  group('admin users matrix', () {
    for (final v in variants()) {
      testWidgets('approvals and users $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        final fake = await _pump(tester, v, AdminFixtures.view);
        expect(fake.calls, [
          const CoreCall('loadAdminUsers', {'query': ''}),
        ]);
        expect(
          find.textContaining(l10n.pendingTitle, findRichText: true),
          findsOneWidget,
        );
        expect(find.text('Sara Nabil'), findsOneWidget);
        expect(find.text('Youssef Kamal'), findsOneWidget);
        // Initials and the request time come from the core.
        expect(find.text('SN'), findsOneWidget);
        expect(
          find.text(l10n.requested(username: 'sara.n', date: '2h ago')),
          findsOneWidget,
        );
        expect(
          find.bySemanticsLabel(l10n.approveSemantics(name: 'Sara Nabil')),
          findsOneWidget,
        );
        if (v.sizeClass == SizeClass.compact) {
          // One pane: cards, then rows (built as they scroll in); creating
          // an account is the app bar's action.
          expect(find.byType(UsersTable), findsNothing);
          expect(find.byType(AppBar), findsOneWidget);
          expect(find.byTooltip(l10n.createAccount), findsOneWidget);
          await tester.scrollUntilVisible(
            find.byType(TextField),
            200,
            scrollable: find.byType(Scrollable).first,
          );
          expect(
            find.bySemanticsLabel(RegExp(RegExp.escape(l10n.searchLabel))),
            findsOneWidget,
          );
          await tester.scrollUntilVisible(
            find.text(l10n.statusDisabled),
            200,
            scrollable: find.byType(Scrollable).first,
          );
          expect(find.byType(UserTile), findsWidgets);
          expect(find.text(l10n.exportNotDownloaded), findsOneWidget);
        } else {
          expect(
            find.textContaining(l10n.allUsers, findRichText: true),
            findsOneWidget,
          );
          expect(find.text(l10n.createAccount), findsOneWidget);
          expect(
            find.bySemanticsLabel(RegExp(RegExp.escape(l10n.searchLabel))),
            findsOneWidget,
          );
          expect(find.text('Karim Adel'), findsOneWidget);
          expect(find.text(l10n.exportNotDownloaded), findsOneWidget);
          expect(find.text(l10n.statusDisabled), findsOneWidget);
          expect(
            find.text(l10n.statusDeletion(date: '11 Oct')),
            findsNWidgets(2),
          );
          expect(find.byType(UsersTable), findsOneWidget);
          expect(find.text(l10n.columnStatus), findsOneWidget);
          // The admin's own row: badge, no actions, no role picker.
          expect(find.text(l10n.you), findsOneWidget);
          expect(find.text(l10n.yourAccount), findsOneWidget);
          expect(find.text('4 Jan'), findsOneWidget);
          expect(find.text(l10n.passwordChangePending), findsOneWidget);
          expect(find.byType(DropdownButton<String>), findsNWidgets(4));
          expect(
            find.byTooltip(l10n.disableSemantics(name: 'Ahmed Samir')),
            findsOneWidget,
          );
          expect(
            find.byTooltip(l10n.disableSemantics(name: 'Shawket')),
            findsNothing,
          );
          expect(
            find.bySemanticsLabel(
              l10n.cancelDeletionSemantics(name: 'Karim Adel'),
            ),
            findsOneWidget,
          );
          expect(
            find.bySemanticsLabel(l10n.enableSemantics(name: 'Mona Hassan')),
            findsOneWidget,
          );
        }
        expect(
          directionOf(tester, find.byType(AdminUsersContent)),
          v.rtl ? TextDirection.rtl : TextDirection.ltr,
        );
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('nobody waiting $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        await _pump(tester, v, AdminFixtures.noPending);
        expect(find.text(l10n.pendingEmpty), findsOneWidget);
        expect(find.byType(PendingUserCard), findsNothing);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('no search match $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        await _pump(tester, v, AdminFixtures.noMatch);
        expect(find.text(l10n.noMatches(query: 'zed')), findsOneWidget);
        expect(find.byType(UserTile), findsNothing);
        expect(find.byType(UsersTable), findsNothing);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('offline $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        await _pump(tester, v, AdminFixtures.unavailable(Availability.offline));
        expect(find.text(l10n.offlineTitle), findsOneWidget);
        expect(find.text(l10n.retry), findsOneWidget);
        expect(find.byTooltip(l10n.createAccount), findsNothing);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });

      testWidgets('error $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        final fake = FakeCoreApi()
          ..loadAdminUsersAnswer.throws(
            const CoreFailure(code: 'server', messageKey: 'error.server'),
          );
        await pumpVariant(tester, v, const AdminUsersScreen(), fake: fake);
        expect(find.text(l10n.loadFailed), findsOneWidget);
        expect(find.text(l10n.errorGeneric(code: 'server')), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: v.textScale == 1);
      });
    }
  });

  group('admin users', () {
    final v = variants().first;
    final expanded = variants().firstWhere((v) => v.sizeName == 'expanded');

    for (final (availability, title) in [
      (Availability.notYetAvailable, 'Not available yet'),
      (Availability.notAllowed, 'Admins only'),
    ]) {
      testWidgets('$availability', (tester) async {
        await _pump(tester, v, AdminFixtures.unavailable(availability));
        expect(find.text(title), findsOneWidget);
        expect(find.byType(AdminUsersContent), findsNothing);
      });
    }

    testWidgets('loading', (tester) async {
      final completer = Completer<AdminUsersView>();
      await pumpVariant(
        tester,
        v,
        const AdminUsersScreen(),
        overrides: [
          adminUsersProvider('').overrideWith((ref) => completer.future),
        ],
      );
      expect(find.bySemanticsLabel('Loading users'), findsOneWidget);
      completer.complete(AdminFixtures.view);
      await settle(tester);
      expect(find.byType(AdminUsersContent), findsOneWidget);
    });

    testWidgets('Retry reloads', (tester) async {
      final fake = await _pump(
        tester,
        v,
        AdminFixtures.unavailable(Availability.offline),
      );
      fake.loadAdminUsersAnswer.returns(AdminFixtures.view);
      await tester.tap(find.text('Retry'));
      await settle(tester);
      expect(fake.calls, [
        const CoreCall('loadAdminUsers', {'query': ''}),
        const CoreCall('loadAdminUsers', {'query': ''}),
      ]);
      expect(find.byType(AdminUsersContent), findsOneWidget);
    });

    testWidgets('embedded: no app bar on compact', (tester) async {
      await _pump(tester, v, AdminFixtures.view, embedded: true);
      expect(find.byType(AppBar), findsNothing);
    });

    testWidgets('search asks the core, keeping the list', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      await tester.enterText(find.byType(TextField), 'mona');
      await settle(tester);
      expect(
        fake.calls.last,
        const CoreCall('loadAdminUsers', {'query': 'mona'}),
      );
      expect(find.byType(UsersTable), findsOneWidget);
    });

    testWidgets('Approve and Reject answer the request', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      await tapVisible(tester, find.bySemanticsLabel('Approve Sara Nabil'));
      expect(find.text('Sara Nabil approved'), findsOneWidget);
      await tapVisible(tester, find.bySemanticsLabel('Reject Youssef Kamal'));
      expect(_intents(fake), [
        const CoreCall('approveUser', {'id': 'u-sara'}),
        const CoreCall('loadAdminUsers', {'query': ''}),
        const CoreCall('rejectUser', {'id': 'u-youssef'}),
        const CoreCall('loadAdminUsers', {'query': ''}),
      ]);
    });

    testWidgets('a failed intent shows the failure', (tester) async {
      final fake = FakeCoreApi()
        ..loadAdminUsersAnswer.returns(AdminFixtures.view)
        ..approveUserAnswer.throws(
          const CoreFailure(code: 'offline', messageKey: 'error.offline'),
        );
      await pumpVariant(
        tester,
        expanded,
        const AdminUsersScreen(),
        fake: fake,
        scaffold: true,
      );
      await tapVisible(tester, find.bySemanticsLabel('Approve Sara Nabil'));
      expect(find.text("You're offline."), findsOneWidget);
      expect(_intents(fake), [
        const CoreCall('approveUser', {'id': 'u-sara'}),
      ]);
    });

    testWidgets('disable, enable, cancel deletion', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      await tapVisible(tester, find.byTooltip('Disable Ahmed Samir'));
      await tapVisible(tester, find.bySemanticsLabel('Enable Mona Hassan'));
      await tapVisible(
        tester,
        find.bySemanticsLabel('Cancel deletion of Karim Adel'),
      );
      expect(_intents(fake).where((c) => c.method != 'loadAdminUsers'), [
        const CoreCall('setUserEnabled', {'id': 'u-ahmed', 'enabled': false}),
        const CoreCall('setUserEnabled', {'id': 'u-mona', 'enabled': true}),
        const CoreCall('cancelDeletion', {'id': 'u-karim'}),
      ]);
    });

    testWidgets('the role picker sets the role', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      expect(find.bySemanticsLabel(RegExp('^Role of Ahmed Samir')), findsOneWidget);
      await tapVisible(tester, find.byType(DropdownButton<String>).first);
      await tester.tap(find.text('Admin').last);
      await settle(tester);
      expect(_intents(fake), [
        const CoreCall('setUserRole', {'id': 'u-ahmed', 'role': 'admin'}),
        const CoreCall('loadAdminUsers', {'query': ''}),
      ]);
    });

    testWidgets('reset password shows the one-time password', (tester) async {
      final fake = FakeCoreApi()
        ..loadAdminUsersAnswer.returns(AdminFixtures.view)
        ..resetPasswordAnswer.returns('tide-4821-sand');
      await pumpVariant(
        tester,
        expanded,
        const AdminUsersScreen(),
        fake: fake,
        scaffold: true,
      );
      await tapVisible(
        tester,
        find.byTooltip('Reset password for Ahmed Samir'),
      );
      expect(find.text('One-time password for @ahmed.s'), findsOneWidget);
      expect(find.text('tide-4821-sand'), findsOneWidget);
      expect(
        _intents(fake).first,
        const CoreCall('resetPassword', {'id': 'u-ahmed'}),
      );
    });

    testWidgets('delete asks first, then schedules', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      await tapVisible(tester, find.byTooltip('Delete Mona Hassan…'));
      expect(find.text('Schedule deletion of @mona.h?'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await settle(tester);
      expect(_intents(fake), isEmpty);
      await tapVisible(tester, find.byTooltip('Delete Mona Hassan…'));
      await tester.tap(find.text('Schedule deletion'));
      await settle(tester);
      expect(_intents(fake), [
        const CoreCall('scheduleDeletion', {'id': 'u-mona'}),
        const CoreCall('loadAdminUsers', {'query': ''}),
      ]);
    });

    testWidgets('create account sends the request', (tester) async {
      final fake = await _pump(tester, expanded, AdminFixtures.view);
      await tapVisible(tester, find.text('Create account'));
      await tester.enterText(
        find.widgetWithText(TextField, 'Username'),
        'hany',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Display name'),
        'Hany Youssef',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Temporary password'),
        'tide-4821-sand',
      );
      await tester.tap(find.text('Create'));
      await settle(tester);
      expect(_intents(fake), [
        const CoreCall('createUser', {
          'request': NewUserRequest(
            username: 'hany',
            displayName: 'Hany Youssef',
            password: 'tide-4821-sand',
            role: 'member',
          ),
        }),
        const CoreCall('loadAdminUsers', {'query': ''}),
      ]);
      expect(find.text('Account @hany created'), findsOneWidget);
    });

    testWidgets('compact rows open the account actions', (tester) async {
      final fake = await _pump(tester, v, AdminFixtures.view);
      await tapVisible(tester, find.text('Ahmed Samir'));
      expect(find.text('Actions for Ahmed Samir'), findsOneWidget);
      expect(find.byTooltip('Reset password for Ahmed Samir'), findsOneWidget);
      await tester.tap(find.text('Make admin'));
      await settle(tester);
      expect(
        _intents(fake).first,
        const CoreCall('setUserRole', {'id': 'u-ahmed', 'role': 'admin'}),
      );
    });

    testWidgets('a downloaded export shows its date', (tester) async {
      await _pump(tester, expanded, AdminFixtures.view);
      expect(find.textContaining('Export downloaded'), findsOneWidget);
    });
  });

  group('admin dialogs', () {
    final v = variants().first;
    final nour = AdminFixtures.view.users[4];

    testWidgets('schedule deletion confirms with true', (tester) async {
      bool? result;
      await pumpVariant(
        tester,
        v,
        Builder(
          builder: (context) => Scaffold(
            body: Center(
              child: FilledButton(
                onPressed: () async => result = await showDialog<bool>(
                  context: context,
                  builder: (_) => ScheduleDeletionDialog(user: nour),
                ),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('open'));
      await settle(tester);
      expect(find.text('Schedule deletion of @nour?'), findsOneWidget);
      expect(
        find.textContaining('deleted when the grace period ends'),
        findsOneWidget,
      );
      await expectAccessible(tester);
      await tester.tap(find.text('Schedule deletion'));
      await settle(tester);
      expect(result, isTrue);
    });

    testWidgets('one-time password is shown once, with copy', (tester) async {
      final copied = <String>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            copied.add((call.arguments as Map)['text'] as String);
          }
          return null;
        },
      );
      await pumpVariant(
        tester,
        v,
        Scaffold(
          body: OneTimePasswordDialog(user: nour, password: 'tide-4821-sand'),
        ),
      );
      expect(find.text('One-time password for @nour'), findsOneWidget);
      expect(find.text('tide-4821-sand'), findsOneWidget);
      await tester.tap(find.text('Copy'));
      await settle(tester);
      expect(copied, ['tide-4821-sand']);
      expect(find.text('Copied'), findsOneWidget);
      await expectAccessible(tester);
    });

    for (final dv in variants(sizes: goldenSizes)) {
      testWidgets('create account dialog $dv', (tester) async {
        await pumpVariant(
          tester,
          dv,
          const Scaffold(body: CreateAccountDialog()),
        );
        final l10n = lookupAdminLocalizations(dv.locale);
        expect(find.text(l10n.createTitle), findsOneWidget);
        expect(find.text(l10n.fieldUsername), findsOneWidget);
        expect(find.text(l10n.roleMember), findsOneWidget);
        expectNoErrors(tester);
        await expectAccessible(tester, contrast: dv.textScale == 1);
      });
    }

    test('labels', () {
      final l10n = lookupAdminLocalizations(const Locale('en'));
      expect(l10n.role('admin'), 'Admin');
      expect(statusTone('deletion_pending'), StatusTone.danger);
      expect(statusTone('active'), StatusTone.success);
      expect(statusIcon('rejected'), Icons.cancel_outlined);
      expect(l10n.failure(Exception()), 'Something went wrong (internal).');
      expect(
        l10n.failure(const CoreFailure(code: 'offline', messageKey: 'e')),
        "You're offline.",
      );
      expect(AdminUsersScreen.icon, Icons.admin_panel_settings_outlined);
    });
  });
}
