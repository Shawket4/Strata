import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Opens the account sheet (SCREEN_SPEC AccountSheetCompact): a bottom
/// sheet on compact, a dialog on medium and expanded.
Future<void> showAccountSheet(
  BuildContext context, {
  VoidCallback? onOpenDevices,
  VoidCallback? onOpenAdminUsers,
}) {
  VoidCallback? close(BuildContext surface, VoidCallback? then) => then == null
      ? null
      : () {
          Navigator.of(surface).pop();
          then();
        };
  if (SizeClass.of(context) == SizeClass.compact) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      showDragHandle: true,
      builder: (sheet) => AccountSheet(
        onOpenDevices: close(sheet, onOpenDevices),
        onOpenAdminUsers: close(sheet, onOpenAdminUsers),
      ),
    );
  }
  return showDialog<void>(
    context: context,
    builder: (dialog) => Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: AccountSheet(
          onOpenDevices: close(dialog, onOpenDevices),
          onOpenAdminUsers: close(dialog, onOpenAdminUsers),
        ),
      ),
    ),
  );
}

/// The account sheet's content: who is signed in, this device, Devices,
/// Admin → Users (admins) and Sign out with the pending-outbox warning
/// (§12.7).
class AccountSheet extends ConsumerWidget {
  /// Creates the sheet.
  const new({super.key, this.onOpenDevices, this.onOpenAdminUsers});

  /// Opens Settings → Devices.
  final VoidCallback? onOpenDevices;

  /// Opens Admin → Users.
  final VoidCallback? onOpenAdminUsers;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final session = ref.watch(sessionProvider).value;
    final account = session?.account;
    final admin = ref.watch(settingsProvider).value?.admin;
    return Semantics(
      label: l10n.accountSheetLabel,
      explicitChildNodes: true,
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(
          StrataSpacing.s5,
          StrataSpacing.s2,
          StrataSpacing.s5,
          StrataSpacing.s5,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            if (account != null)
              Row(
                children: [
                  StrataAvatar(initials: account.initials, size: 56),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Semantics(
                          header: true,
                          container: true,
                          child: Text(
                            account.displayName,
                            style: text.title.copyWith(
                              fontSize: StrataTypeScale.titleSmall + 2,
                            ),
                          ),
                        ),
                        Text(
                          l10n.atUsername(username: account.username),
                          textDirection: TextDirection.ltr,
                          style: text.monoSmall.copyWith(color: colors.text2),
                        ),
                      ],
                    ),
                  ),
                  StatusPill(
                    label: l10n.role(account.role),
                    tone: account.isAdmin
                        ? StatusTone.info
                        : StatusTone.neutral,
                    icon: account.isAdmin
                        ? Icons.shield_outlined
                        : Icons.person_outline,
                  ),
                ],
              ),
            if (session != null && session.deviceName.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s4),
              Container(
                padding: const EdgeInsets.all(StrataSpacing.s3),
                decoration: BoxDecoration(
                  color: colors.surface2,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: MergeSemantics(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l10n.thisDevice,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                      Text(switch (session.thisDevice) {
                        final device? => l10n.thisDeviceDetail(
                          name: device.name,
                          signedIn: device.signedInLabel,
                        ),
                        null => session.deviceName,
                      }, style: text.bodyStrong),
                    ],
                  ),
                ),
              ),
            ],
            const SizedBox(height: StrataSpacing.s3),
            _NavRow(
              icon: Icons.devices_outlined,
              label: l10n.devices,
              detail: session?.deviceCount?.toString(),
              onTap: onOpenDevices,
            ),
            if (admin == Availability.available)
              _NavRow(
                icon: Icons.admin_panel_settings_outlined,
                label: l10n.adminUsers,
                detail: switch (session?.pendingApprovals) {
                  final count? => l10n.pendingCount(count: count),
                  null => null,
                },
                onTap: onOpenAdminUsers,
              ),
            Divider(height: StrataSpacing.s4, color: colors.border),
            _NavRow(
              icon: Icons.logout,
              label: l10n.signOut,
              danger: true,
              onTap: () => unawaited(signOutFlow(context, ref)),
            ),
          ],
        ),
      ),
    );
  }
}

class _NavRow extends StatelessWidget {
  const new({
    required this.icon,
    required this.label,
    required this.onTap,
    this.danger = false,
    this.detail,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;
  final bool danger;
  final String? detail;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final color = danger ? colors.dangerText : colors.text;
    final extra = detail;
    return Semantics(
      button: true,
      child: InkWell(
        onTap: onTap,
        child: ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 52),
          child: Row(
            children: [
              Icon(icon, color: color),
              const SizedBox(width: StrataSpacing.s4),
              Expanded(
                child: Text(label, style: text.body.copyWith(color: color)),
              ),
              if (extra != null) ...[
                const SizedBox(width: StrataSpacing.s2),
                Flexible(
                  child: Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: Text(
                      extra,
                      textAlign: TextAlign.end,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  ),
                ),
              ],
              if (!danger) ...[
                const SizedBox(width: StrataSpacing.s1),
                Icon(Icons.chevron_right, color: colors.text2),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// What the user chose in the sign-out warning.
enum SignOutChoice {
  /// Sync first ("Sync now").
  syncNow,

  /// Sign out and drop the unsynced changes.
  signOutAnyway,

  /// Keep the account.
  cancel,
}

/// Signs out (§12.7): `signOut(force: false)`; when the core reports
/// unsynced ops, asks "Sync now / Sign out anyway / Cancel" and forwards the
/// choice (`syncNow` or `signOut(force: true)`).
Future<void> signOutFlow(BuildContext context, WidgetRef ref) async {
  final api = ref.read(coreApiProvider);
  final l10n = context.accountsL10n;
  final messenger = ScaffoldMessenger.maybeOf(context);
  try {
    final outcome = await api.signOut(force: false);
    if (outcome.signedOut || !context.mounted) return;
    final choice = await showDialog<SignOutChoice>(
      context: context,
      builder: (_) => SignOutWarningDialog(unsyncedOps: outcome.unsyncedOps),
    );
    switch (choice) {
      case SignOutChoice.syncNow:
        await api.syncNow();
      case SignOutChoice.signOutAnyway:
        await api.signOut(force: true);
      case SignOutChoice.cancel || null:
        break;
    }
  } on CoreFailure catch (error) {
    messenger?.showSnackBar(SnackBar(content: Text(l10n.failure(error))));
  }
}

/// "3 changes haven't synced yet" (SCREEN_SPEC AccountSheetCompact dialog),
/// listing the queued ops from the sync status.
class SignOutWarningDialog extends ConsumerWidget {
  /// Creates the dialog for [unsyncedOps] ops.
  const new({required this.unsyncedOps, super.key});

  /// Unsynced ops, as the core reported them.
  final int unsyncedOps;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final sync = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final outbox = ref.watch(syncStatusProvider).value?.outbox ?? const [];
    void pick(SignOutChoice choice) => Navigator.of(context).pop(choice);
    return AlertDialog(
      icon: Icon(Icons.cloud_upload_outlined, color: colors.warningText),
      title: Text(l10n.unsyncedTitle(count: unsyncedOps)),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(l10n.unsyncedBody, style: text.body),
            if (outbox.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s3),
              for (final item in outbox)
                MergeSemantics(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      vertical: StrataSpacing.s1,
                    ),
                    child: Row(
                      children: [
                        Icon(Icons.circle, size: 6, color: colors.text2),
                        const SizedBox(width: StrataSpacing.s2),
                        Expanded(
                          child: Text(
                            item.title ?? SyncLabels.kind(sync, item.kind),
                            style: text.bodySmall.withWeight(FontWeight.w600),
                          ),
                        ),
                        const SizedBox(width: StrataSpacing.s2),
                        Flexible(
                          child: Text(
                            SyncLabels.kind(sync, item.kind),
                            textAlign: TextAlign.end,
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ],
        ),
      ),
      actionsOverflowDirection: VerticalDirection.down,
      actionsOverflowButtonSpacing: StrataSpacing.s2,
      actions: [
        TextButton(
          onPressed: () => pick(SignOutChoice.cancel),
          child: Text(l10n.cancel),
        ),
        TextButton(
          onPressed: () => pick(SignOutChoice.signOutAnyway),
          style: TextButton.styleFrom(foregroundColor: colors.dangerText),
          child: Text(l10n.signOutAnyway),
        ),
        FilledButton(
          onPressed: () => pick(SignOutChoice.syncNow),
          child: Text(l10n.syncNow),
        ),
      ],
    );
  }
}
