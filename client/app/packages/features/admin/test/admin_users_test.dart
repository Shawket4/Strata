import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_admin/strata_admin.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

import 'helpers/fixtures.dart';
import 'helpers/matrix.dart';

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
  );
  return fake;
}

void main() {
  group('admin users matrix', () {
    for (final v in matrix()) {
      testWidgets('approvals and users $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        final fake = await _pump(tester, v, AdminFixtures.view);
        expect(fake.calls, [const CoreCall('loadAdminUsers')]);
        expect(
          find.textContaining(l10n.pendingTitle, findRichText: true),
          findsOneWidget,
        );
        expect(find.text('Sara Nabil'), findsOneWidget);
        expect(find.text('Youssef Kamal'), findsOneWidget);
        expect(
          find.bySemanticsLabel(l10n.approveSemantics(name: 'Sara Nabil')),
          findsOneWidget,
        );
        if (v.sizeClass == SizeClass.compact) {
          // One pane: cards, then rows (built as they scroll in).
          expect(find.byType(UsersTable), findsNothing);
          expect(find.byType(AppBar), findsOneWidget);
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
          expect(find.text('Karim Adel'), findsOneWidget);
          expect(find.text(l10n.exportNotDownloaded), findsOneWidget);
          expect(find.text(l10n.statusDisabled), findsOneWidget);
          expect(find.byType(UsersTable), findsOneWidget);
          expect(find.text(l10n.columnStatus), findsOneWidget);
          expect(
            find.byTooltip(l10n.disableSemantics(name: 'Ahmed Samir')),
            findsOneWidget,
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

      testWidgets('offline $v', (tester) async {
        final l10n = lookupAdminLocalizations(v.locale);
        await _pump(tester, v, AdminFixtures.unavailable(Availability.offline));
        expect(find.text(l10n.offlineTitle), findsOneWidget);
        expect(find.text(l10n.retry), findsOneWidget);
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
    final v = matrix().first;
    final expanded = matrix().firstWhere((v) => v.sizeName == 'expanded');

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
        overrides: [adminUsersProvider.overrideWith((ref) => completer.future)],
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
      expect(
        fake.calls.where((c) => c.method == 'loadAdminUsers'),
        hasLength(2),
      );
      expect(find.byType(AdminUsersContent), findsOneWidget);
    });

    testWidgets('embedded: no app bar on compact', (tester) async {
      await _pump(tester, v, AdminFixtures.view, embedded: true);
      expect(find.byType(AppBar), findsNothing);
    });

    testWidgets('actions the core lacks are disabled', (tester) async {
      await _pump(tester, expanded, AdminFixtures.view);
      for (final button in tester.widgetList<ButtonStyleButton>(
        find.descendant(
          of: find.byType(AdminUsersContent),
          matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
        ),
      )) {
        expect(button.onPressed, isNull);
      }
      for (final button in tester.widgetList<IconButton>(
        find.byType(IconButton),
      )) {
        expect(button.onPressed, isNull);
      }
    });

    testWidgets('compact rows open the account actions', (tester) async {
      await _pump(tester, v, AdminFixtures.view);
      await tapVisible(tester, find.text('Ahmed Samir'));
      expect(find.text('Actions for Ahmed Samir'), findsOneWidget);
      expect(find.byTooltip('Reset password for Ahmed Samir'), findsOneWidget);
    });

    testWidgets('a downloaded export shows its date', (tester) async {
      await _pump(tester, expanded, AdminFixtures.view);
      expect(find.textContaining('Export downloaded'), findsOneWidget);
    });
  });

  group('admin dialogs', () {
    final v = matrix().first;
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
                  builder: (_) => ScheduleDeletionDialog(
                    user: nour,
                    date: DateTime.utc(2026, 10, 11, 9),
                  ),
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
      expect(find.textContaining('Sunday, October 11, 2026'), findsOneWidget);
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
