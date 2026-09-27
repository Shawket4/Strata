import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/src/account_sheet.dart';
import 'package:strata_accounts/src/auth_layout.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// The frame of the restricted account screens: a page on compact; a
/// centred alert card over the dimmed, banded background on medium and
/// expanded (SCREEN_SPEC AccountDisabled*/DeletionPending*).
class RestrictedLayout extends StatelessWidget {
  /// Creates the frame.
  const new({required this.child, required this.caption, super.key});

  /// The card or page content.
  final Widget child;

  /// Header caption (server, account).
  final String caption;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final header = Row(
      children: [
        const StrataWordmark(fontSize: 18),
        const SizedBox(width: StrataSpacing.s3),
        Expanded(
          child: Text(
            caption,
            textAlign: TextAlign.end,
            textDirection: TextDirection.ltr,
            overflow: TextOverflow.ellipsis,
            style: text.monoSmall.copyWith(color: colors.text2),
          ),
        ),
      ],
    );
    if (SizeClass.of(context) == SizeClass.compact) {
      return Scaffold(
        backgroundColor: colors.background,
        body: SafeArea(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
              StrataSpacing.s6,
              StrataSpacing.s4,
              StrataSpacing.s6,
              StrataSpacing.s6,
            ),
            children: [
              header,
              const SizedBox(height: StrataSpacing.s8),
              child,
            ],
          ),
        ),
      );
    }
    return Scaffold(
      backgroundColor: colors.surface2,
      body: Stack(
        children: [
          const Positioned.fill(child: ExcludeSemantics(child: StrataBands())),
          Positioned.fill(child: ColoredBox(color: colors.scrim)),
          SafeArea(
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(StrataSpacing.s6),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 560),
                  child: Semantics(
                    scopesRoute: true,
                    namesRoute: true,
                    explicitChildNodes: true,
                    child: Material(
                      color: colors.surface,
                      borderRadius: const BorderRadius.all(
                        Radius.circular(StrataRadii.sheet),
                      ),
                      child: Padding(
                        padding: const EdgeInsets.all(StrataSpacing.s6),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            header,
                            const SizedBox(height: StrataSpacing.s5),
                            child,
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Illustration extends StatelessWidget {
  const new({required this.icon, required this.danger});

  final IconData icon;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return ExcludeSemantics(
      child: Align(
        alignment: AlignmentDirectional.centerStart,
        child: Container(
          width: 56,
          height: 56,
          decoration: BoxDecoration(
            color: danger ? colors.dangerTint : colors.warningTint,
            shape: BoxShape.circle,
          ),
          child: Icon(
            icon,
            size: 28,
            color: danger ? colors.dangerText : colors.warningText,
          ),
        ),
      ),
    );
  }
}

/// An action the core cannot perform yet (see docs/CORE_GAPS.md): shown
/// disabled with a "Not available yet" tooltip.
class _PendingAction extends StatelessWidget {
  const new({required this.label, required this.icon, this.filled = false});

  final String label;
  final IconData icon;
  final bool filled;

  @override
  Widget build(BuildContext context) {
    final l10n = context.accountsL10n;
    const style = ButtonStyle(
      minimumSize: WidgetStatePropertyAll(
        Size.fromHeight(StrataLayout.minTouchTarget),
      ),
    );
    return Tooltip(
      message: l10n.notAvailableYet,
      child: filled
          ? FilledButton.icon(
              onPressed: null,
              style: style,
              icon: Icon(icon, size: 20),
              label: Text(label),
            )
          : OutlinedButton.icon(
              onPressed: null,
              style: style,
              icon: Icon(icon, size: 20),
              label: Text(label),
            ),
    );
  }
}

/// The queued ops that never synced (titles and kinds from the sync
/// status).
class _UnsyncedList extends ConsumerWidget {
  const new({required this.count});

  final int count;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final sync = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final outbox = ref.watch(syncStatusProvider).value?.outbox ?? const [];
    return Container(
      decoration: BoxDecoration(
        color: colors.surface,
        borderRadius: StrataRadii.cardRadius,
        border: Border.all(color: colors.border),
      ),
      padding: const EdgeInsets.all(StrataSpacing.s4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: StrataSpacing.s2,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Semantics(
                header: true,
                child: Text(
                  l10n.unsyncedCount(count: count),
                  style: text.bodyStrong,
                ),
              ),
              StatusPill(label: l10n.onlyOnDevice, tone: StatusTone.warning),
            ],
          ),
          for (final item in outbox)
            MergeSemantics(
              child: Padding(
                padding: const EdgeInsets.only(top: StrataSpacing.s2),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      item.title ?? SyncLabels.kind(sync, item.kind),
                      textAlign: TextAlign.start,
                      style: text.bodyStrong,
                    ),
                    Text(
                      l10n.unsyncedItem(
                        kind: SyncLabels.kind(sync, item.kind),
                        time: formatSyncClock(context, item.created),
                      ),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The account was disabled by an admin (§12.7, SCREEN_SPEC
/// AccountDisabled*): local data is removed after the user acknowledges
/// (`acknowledgeAccountDisabled`).
class AccountDisabledScreen extends HookConsumerWidget {
  /// Creates the screen.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final session = ref.watch(sessionProvider).value;
    final account = session?.account;
    final unsynced = session?.unsyncedOps ?? 0;
    final busy = useState(false);

    Future<void> acknowledge() async {
      busy.value = true;
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        await ref.read(coreApiProvider).acknowledgeAccountDisabled();
      } on CoreFailure catch (error) {
        messenger?.showSnackBar(SnackBar(content: Text(l10n.failure(error))));
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

    return RestrictedLayout(
      caption: account?.serverUrl ?? '',
      child: Semantics(
        liveRegion: true,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const _Illustration(icon: Icons.person_off_outlined, danger: true),
            const SizedBox(height: StrataSpacing.s4),
            Semantics(
              header: true,
              child: Text(l10n.disabledTitle, style: text.display),
            ),
            const SizedBox(height: StrataSpacing.s2),
            Text(
              l10n.disabledBody(username: account?.username ?? ''),
              style: text.body,
            ),
            const SizedBox(height: StrataSpacing.s2),
            Text(
              l10n.disabledServerNote,
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
            if (unsynced > 0) ...[
              const SizedBox(height: StrataSpacing.s5),
              _UnsyncedList(count: unsynced),
              const SizedBox(height: StrataSpacing.s4),
              _PendingAction(
                label: l10n.exportFirst,
                icon: Icons.download_outlined,
                filled: true,
              ),
              const SizedBox(height: StrataSpacing.s1),
              Text(
                l10n.exportFirstNote,
                style: text.caption.copyWith(color: colors.text2),
              ),
            ],
            const SizedBox(height: StrataSpacing.s3),
            PrimaryButton(
              label: l10n.removeAndSignOut,
              danger: true,
              busy: busy.value,
              onPressed: acknowledge,
            ),
          ],
        ),
      ),
    );
  }
}

/// The account is scheduled for deletion (§11 screen 15, §12.7, SCREEN_SPEC
/// DeletionPending*): days remaining, export download, unsynced ops, delete
/// now. Local data is read-only.
class DeletionPendingScreen extends ConsumerWidget {
  /// Creates the screen.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final session = ref.watch(sessionProvider).value;
    final account = session?.account;
    final days = session?.daysRemaining;
    final at = session?.deletionAt;
    final unsynced = session?.unsyncedOps ?? 0;
    return RestrictedLayout(
      caption: account?.serverUrl ?? '',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              const _Illustration(
                icon: Icons.auto_delete_outlined,
                danger: false,
              ),
              const SizedBox(width: StrataSpacing.s3),
              if (days != null)
                StatusPill(
                  label: l10n.daysLeft(count: days),
                  tone: StatusTone.warning,
                  icon: Icons.schedule,
                ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s4),
          Semantics(
            header: true,
            child: Text(l10n.deletionTitle, style: text.display),
          ),
          const SizedBox(height: StrataSpacing.s2),
          Text(
            l10n.deletionBody(
              username: account?.username ?? '',
              date: at == null
                  ? ''
                  : MaterialLocalizations.of(context)
                        .formatFullDate(at.toLocal()),
            ),
            style: text.body,
          ),
          const SizedBox(height: StrataSpacing.s5),
          _PendingAction(
            label: l10n.downloadExport,
            icon: Icons.download_outlined,
            filled: true,
          ),
          if (unsynced > 0) ...[
            const SizedBox(height: StrataSpacing.s5),
            Container(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              decoration: BoxDecoration(
                color: colors.warningTint,
                borderRadius: StrataRadii.cardRadius,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Semantics(
                    header: true,
                    child: Text(
                      l10n.unsyncedNotInExport(count: unsynced),
                      style: text.bodyStrong.copyWith(
                        color: colors.warningText,
                      ),
                    ),
                  ),
                  Text(
                    l10n.notInExport,
                    style: text.bodySmall.copyWith(color: colors.text),
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  _PendingAction(
                    label: l10n.saveAsFile,
                    icon: Icons.save_alt_outlined,
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: StrataSpacing.s5),
          Divider(height: 1, color: colors.border),
          const SizedBox(height: StrataSpacing.s3),
          Text(
            l10n.readOnlyNote,
            style: text.caption.copyWith(color: colors.text2),
          ),
          const SizedBox(height: StrataSpacing.s3),
          _PendingAction(label: l10n.deleteNow, icon: Icons.delete_outline),
          const SizedBox(height: StrataSpacing.s2),
          TextButton(
            onPressed: () => unawaited(signOutFlow(context, ref)),
            child: Text(l10n.signOut),
          ),
        ],
      ),
    );
  }
}

/// An admin reset the password: a new one is needed before anything else
/// (`SessionKind.passwordChangeRequired`).
class PasswordChangeRequiredScreen extends HookConsumerWidget {
  /// Creates the screen.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final account = ref.watch(sessionProvider).value?.account;
    final password = useTextEditingController();
    final confirm = useTextEditingController();
    return RestrictedLayout(
      caption: account?.serverUrl ?? '',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const _Illustration(icon: Icons.password, danger: false),
          const SizedBox(height: StrataSpacing.s4),
          Semantics(
            header: true,
            child: Text(l10n.passwordChangeTitle, style: text.display),
          ),
          const SizedBox(height: StrataSpacing.s2),
          Text(
            l10n.passwordChangeBody,
            style: text.body.copyWith(color: colors.text2),
          ),
          const SizedBox(height: StrataSpacing.s5),
          LabeledField(
            label: l10n.fieldNewPassword,
            controller: password,
            obscure: true,
          ),
          const SizedBox(height: StrataSpacing.s4),
          LabeledField(
            label: l10n.fieldConfirmPassword,
            controller: confirm,
            obscure: true,
            textInputAction: TextInputAction.done,
          ),
          const SizedBox(height: StrataSpacing.s5),
          _PendingAction(
            label: l10n.changePassword,
            icon: Icons.check,
            filled: true,
          ),
          const SizedBox(height: StrataSpacing.s2),
          TextButton(
            onPressed: () => unawaited(signOutFlow(context, ref)),
            child: Text(l10n.signOut),
          ),
        ],
      ),
    );
  }
}
