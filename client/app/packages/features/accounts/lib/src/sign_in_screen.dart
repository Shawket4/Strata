import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/src/auth_layout.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Sign in (§11 screen 1, SCREEN_SPEC LoginCompact / LoginExpanded):
/// username, password and device name (prefilled from the signed-out
/// session); accounts with data on this device can be resumed. There is no
/// server field: the core signs in to the build's server
/// (`STRATA_SERVER_URL`).
///
/// A successful sign-in changes the session stream (the app routes on it);
/// an account waiting for approval (or refused) becomes a session state of
/// its own, which the app routes to the approval screen
/// ([onPendingApproval] is called too).
class SignInScreen extends HookConsumerWidget {
  /// Creates the sign-in screen.
  const new({super.key, this.onCreateAccount, this.onPendingApproval});

  /// The icon that represents this feature.
  static const IconData icon = Icons.login;

  /// Opens sign-up.
  final VoidCallback? onCreateAccount;

  /// The account exists but is not approved (or was turned down).
  final VoidCallback? onPendingApproval;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final session = ref.watch(sessionProvider).value;
    final username = useTextEditingController();
    final password = useTextEditingController();
    final device = useTextEditingController(text: session?.deviceName ?? '');
    // The session may arrive after the first build: prefill once.
    useEffect(() {
      if (device.text.isEmpty) device.text = session?.deviceName ?? '';
      return null;
    }, [session?.deviceName]);
    final obscured = useState(true);
    final busy = useState(false);
    final failure = useState<String?>(null);
    final sizeClass = SizeClass.of(context);
    final wide = sizeClass != SizeClass.compact;

    Future<void> submit() async {
      if (busy.value) return;
      busy.value = true;
      failure.value = null;
      final request = SignInRequest(
        username: username.text,
        password: password.text,
        deviceName: device.text,
      );
      try {
        await ref.read(coreApiProvider).signIn(request: request);
      } on CoreFailure catch (error) {
        if (error.code == 'account_pending' ||
            error.code == 'account_rejected') {
          onPendingApproval?.call();
        } else {
          failure.value = l10n.failure(error);
        }
      } on Object catch (error, stack) {
        reportUntypedFailure(error, stack, action: 'signing in');
        failure.value = l10n.errorUnreachable;
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

    Future<void> resume(String userId) async {
      failure.value = null;
      try {
        await ref.read(coreApiProvider).switchAccount(userId: userId);
      } on CoreFailure catch (error) {
        failure.value = l10n.failure(error);
      } on Object catch (error, stack) {
        reportUntypedFailure(error, stack, action: 'switching account');
        failure.value = l10n.errorUnreachable;
      }
    }

    final known = session?.knownAccounts ?? const <KnownAccountItem>[];
    final error = failure.value;
    return AuthLayout(
      child: AutofillGroup(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (sizeClass != SizeClass.expanded) ...[
              const Align(
                alignment: AlignmentDirectional.centerStart,
                child: StrataWordmark(fontSize: 26),
              ),
              const SizedBox(height: StrataSpacing.s8),
            ],
            Semantics(
              header: true,
              container: true,
              child: Text(l10n.signInTitle, style: text.display),
            ),
            const SizedBox(height: StrataSpacing.s1),
            Text(
              wide ? l10n.signInSubtitleWide : l10n.signInSubtitle,
              style: text.body.copyWith(
                fontSize: StrataTypeScale.bodyDense,
                color: colors.text2,
              ),
            ),
            const SizedBox(height: StrataSpacing.s6),
            LabeledField(
              label: l10n.fieldUsername,
              controller: username,
              autofillHints: const [AutofillHints.username],
            ),
            const SizedBox(height: StrataSpacing.s4),
            LabeledField(
              label: l10n.fieldPassword,
              controller: password,
              obscure: obscured.value,
              autofillHints: const [AutofillHints.password],
              suffix: PasswordVisibilityButton(
                obscured: obscured.value,
                onToggle: () => obscured.value = !obscured.value,
              ),
            ),
            if (error != null) ...[
              const SizedBox(height: StrataSpacing.s2),
              FormAlert(message: error),
            ],
            const SizedBox(height: StrataSpacing.s4),
            LabeledField(
              label: l10n.fieldDeviceName,
              controller: device,
              helper: l10n.deviceNameHint,
              textInputAction: TextInputAction.done,
              onSubmitted: (_) => submit(),
            ),
            const SizedBox(height: StrataSpacing.s5),
            PrimaryButton(
              label: l10n.signInTitle,
              busy: busy.value,
              onPressed: submit,
            ),
            if (wide) ...[
              const SizedBox(height: StrataSpacing.s2),
              Wrap(
                alignment: WrapAlignment.center,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  const KeyboardHintChip(keys: ['Enter']),
                  const SizedBox(width: StrataSpacing.s2),
                  Text(
                    l10n.enterToSignIn,
                    style: text.caption.copyWith(color: colors.text2),
                  ),
                ],
              ),
            ],
            const SizedBox(height: StrataSpacing.s5),
            Divider(height: 1, color: colors.border),
            const SizedBox(height: StrataSpacing.s3),
            Wrap(
              alignment: WrapAlignment.center,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  l10n.newHere,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
                TextButton(
                  onPressed: onCreateAccount,
                  child: Text(l10n.createAccount),
                ),
              ],
            ),
            Text(
              l10n.forgotPassword,
              textAlign: TextAlign.center,
              style: text.caption.copyWith(color: colors.text2),
            ),
            if (known.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s6),
              StrataSectionHeader(
                title: l10n.knownAccountsTitle,
                padding: EdgeInsets.zero,
              ),
              const SizedBox(height: StrataSpacing.s2),
              for (final account in known)
                _KnownAccountTile(
                  account: account,
                  onTap: () => resume(account.userId),
                ),
            ],
          ],
        ),
      ),
    );
  }
}

class _KnownAccountTile extends StatelessWidget {
  const new({required this.account, required this.onTap});

  final KnownAccountItem account;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.accountsL10n;
    return Card(
      margin: const EdgeInsets.only(bottom: StrataSpacing.s2),
      child: ListTile(
        onTap: onTap,
        minTileHeight: StrataLayout.minTouchTarget + StrataSpacing.s2,
        leading: StrataAvatar(initials: account.initials),
        title: Text(l10n.continueAs(name: account.displayName)),
        trailing: const Icon(Icons.chevron_right),
      ),
    );
  }
}
