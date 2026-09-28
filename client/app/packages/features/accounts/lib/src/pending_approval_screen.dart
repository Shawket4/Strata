import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/src/auth_layout.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// "Waiting for approval" after sign-up or a sign-in of a pending account
/// (SCREEN_SPEC PendingApprovalCompact), with the "not approved" variant.
///
/// Rendered from the session: `SessionKind.pendingApproval` /
/// `SessionKind.rejected` with its [PendingApproval] (who, when it was
/// requested and last checked). "Check again" asks the core
/// (`check_approval`, which retries the sign-in it keeps in memory); "Use a
/// different account" leaves the screen (`dismiss_pending`). The app routes
/// on the session that follows.
class PendingApprovalScreen extends HookConsumerWidget {
  /// Creates the screen.
  const new({super.key, this.onUseAnotherAccount});

  /// Called after the pending request was dismissed (the router also
  /// follows the session).
  final VoidCallback? onUseAnotherAccount;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final session = ref.watch(sessionProvider).value;
    final pending = session?.pending;
    final rejected = session?.kind == SessionKind.rejected;
    final busy = useState(false);

    Future<void> checkAgain() async {
      if (busy.value) return;
      busy.value = true;
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        await ref.read(coreApiProvider).checkApproval();
      } on CoreFailure catch (error) {
        final message = switch (error.code) {
          'account_pending' => l10n.stillPending,
          // The session turns into the "not approved" variant.
          'account_rejected' => null,
          _ => l10n.failure(error),
        };
        if (message != null) {
          messenger
            ?..hideCurrentSnackBar()
            ..showSnackBar(SnackBar(content: Text(message)));
        }
      } on Object catch (error, stack) {
        reportUntypedFailure(error, stack, action: 'checking approval');
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.errorUnreachable)));
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

    Future<void> useAnother() async {
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        await ref.read(coreApiProvider).dismissPending();
        onUseAnotherAccount?.call();
      } on CoreFailure catch (error) {
        messenger?.showSnackBar(SnackBar(content: Text(l10n.failure(error))));
      } on Object catch (error, stack) {
        reportUntypedFailure(error, stack, action: 'dismissing the request');
        messenger?.showSnackBar(SnackBar(content: Text(l10n.errorUnreachable)));
      }
    }

    final checked = pending?.lastCheckedLabel;
    return AuthLayout(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Align(
            alignment: AlignmentDirectional.centerStart,
            child: StrataWordmark(fontSize: 18),
          ),
          const SizedBox(height: StrataSpacing.s8),
          if (rejected)
            _Rejected(onUseAnotherAccount: useAnother)
          else ...[
            ExcludeSemantics(
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: Container(
                  width: 68,
                  height: 68,
                  decoration: BoxDecoration(
                    color: colors.accentTint,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(
                    Icons.hourglass_top_rounded,
                    size: 32,
                    color: colors.accentText,
                  ),
                ),
              ),
            ),
            const SizedBox(height: StrataSpacing.s5),
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: StatusPill(
                label: l10n.pendingPill,
                tone: StatusTone.warning,
                icon: Icons.schedule,
              ),
            ),
            const SizedBox(height: StrataSpacing.s3),
            Semantics(
              header: true,
              container: true,
              child: Text(l10n.pendingTitle, style: text.display),
            ),
            const SizedBox(height: StrataSpacing.s2),
            if (pending != null)
              Text(
                l10n.pendingBody(
                  username: pending.username,
                  requested: pending.requestedLabel,
                ),
                style: text.body.copyWith(color: colors.text),
              ),
            const SizedBox(height: StrataSpacing.s2),
            Text(
              l10n.pendingNext,
              style: text.body.copyWith(color: colors.text2),
            ),
            const SizedBox(height: StrataSpacing.s6),
            PrimaryButton(
              label: l10n.checkAgain,
              icon: Icons.refresh,
              busy: busy.value,
              onPressed: pending?.canCheck ?? false ? checkAgain : null,
            ),
            const SizedBox(height: StrataSpacing.s3),
            OutlinedButton(
              onPressed: useAnother,
              style: OutlinedButton.styleFrom(
                minimumSize: const Size.fromHeight(StrataLayout.minTouchTarget),
              ),
              child: Text(l10n.useAnotherAccount),
            ),
            if (checked != null) ...[
              const SizedBox(height: StrataSpacing.s3),
              Semantics(
                liveRegion: true,
                child: Text(
                  checked,
                  textAlign: TextAlign.center,
                  style: text.caption.copyWith(color: colors.text2),
                ),
              ),
            ],
          ],
        ],
      ),
    );
  }
}

class _Rejected extends StatelessWidget {
  const new({required this.onUseAnotherAccount});

  final VoidCallback onUseAnotherAccount;

  @override
  Widget build(BuildContext context) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Container(
      padding: const EdgeInsets.all(StrataSpacing.s4),
      decoration: BoxDecoration(
        color: colors.dangerTint,
        borderRadius: StrataRadii.cardRadius,
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.block, color: colors.dangerText),
          const SizedBox(width: StrataSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Semantics(
                  header: true,
                  container: true,
                  child: Text(
                    l10n.rejectedTitle,
                    style: text.bodyStrong.copyWith(color: colors.dangerText),
                  ),
                ),
                const SizedBox(height: StrataSpacing.s1),
                Text(
                  l10n.rejectedBody,
                  style: text.bodySmall.copyWith(color: colors.text),
                ),
                const SizedBox(height: StrataSpacing.s2),
                TextButton(
                  onPressed: onUseAnotherAccount,
                  child: Text(l10n.useAnotherAccount),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
