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
                  ExcludeSemantics(
                    child: CircleAvatar(
                      radius: 28,
                      backgroundColor: colors.accentTint,
                      foregroundColor: colors.accentText,
                      child: const Icon(Icons.person_outline, size: 28),
                    ),
                  ),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Semantics(
                          header: true,
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
                      Text(session.deviceName, style: text.bodyStrong),
                    ],
                  ),
                ),
              ),
            ],
            const SizedBox(height: StrataSpacing.s3),
            _NavRow(
              icon: Icons.devices_outlined,
              label: l10n.devices,
              onTap: onOpenDevices,
            ),
            if (admin == Availability.available)
              _NavRow(
                icon: Icons.admin_panel_settings_outlined,
                label: l10n.adminUsers,
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
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final color = danger ? colors.dangerText : colors.text;
    return ListTile(
      contentPadding: EdgeInsets.zero,
      minTileHeight: 52,
      leading: Icon(icon, color: color),
      title: Text(label, style: context.strataText.body.copyWith(color: color)),
      trailing: danger ? null : const Icon(Icons.chevron_right),
      onTap: onTap,
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
                        Text(
                          SyncLabels.kind(sync, item.kind),
                          style: text.caption.copyWith(color: colors.text2),
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
