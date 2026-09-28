import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/src/auth_layout.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Sign up (SCREEN_SPEC SignupCompact): display name, username, password
/// with the core's strength meter and confirmation, and the approval notice
/// (D22), on the build's server (no server field). A registered account waits for approval: the session becomes
/// `pendingApproval` (the app routes on it) and [onRequested] is called.
class SignUpScreen extends HookConsumerWidget {
  /// Creates the sign-up screen.
  const new({super.key, this.onBack, this.onRequested});

  /// Back to sign in.
  final VoidCallback? onBack;

  /// The account was registered and waits for approval.
  final VoidCallback? onRequested;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final name = useTextEditingController();
    final username = useTextEditingController();
    final password = useTextEditingController();
    final confirm = useTextEditingController();
    useListenable(password);
    useListenable(confirm);
    final obscured = useState(true);
    final busy = useState(false);
    final failure = useState<String?>(null);
    // Input check of the two password fields only (ephemeral form state).
    final confirmed = confirm.text.isNotEmpty && confirm.text == password.text;

    Future<void> submit() async {
      if (busy.value || !confirmed) return;
      busy.value = true;
      failure.value = null;
      try {
        await ref
            .read(coreApiProvider)
            .signUp(
              request: SignUpRequest(
                username: username.text,
                password: password.text,
                displayName: name.text,
              ),
            );
        onRequested?.call();
      } on CoreFailure catch (error) {
        failure.value = l10n.failure(error);
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

    final back = onBack;
    final error = failure.value;
    return AuthLayout(
      header: back == null
          ? null
          : Align(
              alignment: AlignmentDirectional.centerStart,
              child: Padding(
                padding: const EdgeInsets.all(StrataSpacing.s1),
                child: IconButton(
                  tooltip: l10n.backToSignIn,
                  onPressed: back,
                  icon: const BackButtonIcon(),
                ),
              ),
            ),
      child: AutofillGroup(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Semantics(
              header: true,
              container: true,
              child: Text(l10n.createAccount, style: text.display),
            ),
            const SizedBox(height: StrataSpacing.s6),
            LabeledField(
              label: l10n.fieldDisplayName,
              controller: name,
              autofillHints: const [AutofillHints.name],
            ),
            const SizedBox(height: StrataSpacing.s4),
            LabeledField(
              label: l10n.fieldUsername,
              controller: username,
              prefixText: '@',
              mono: true,
              autofillHints: const [AutofillHints.newUsername],
            ),
            const SizedBox(height: StrataSpacing.s4),
            LabeledField(
              label: l10n.fieldPassword,
              controller: password,
              obscure: obscured.value,
              autofillHints: const [AutofillHints.newPassword],
              suffix: PasswordVisibilityButton(
                obscured: obscured.value,
                onToggle: () => obscured.value = !obscured.value,
              ),
            ),
            if (password.text.isNotEmpty) ...[
              const SizedBox(height: StrataSpacing.s2),
              PasswordStrengthMeter(password: password.text),
            ],
            const SizedBox(height: StrataSpacing.s4),
            LabeledField(
              label: l10n.fieldConfirmPassword,
              controller: confirm,
              obscure: obscured.value,
              textInputAction: TextInputAction.done,
              onSubmitted: (_) => submit(),
              helper: confirmed ? l10n.passwordsMatch : null,
              error: confirm.text.isNotEmpty && !confirmed
                  ? l10n.passwordsDiffer
                  : null,
            ),
            const SizedBox(height: StrataSpacing.s4),
            Semantics(
              container: true,
              child: Container(
                padding: const EdgeInsets.all(StrataSpacing.s3),
                decoration: BoxDecoration(
                  color: colors.infoTint,
                  borderRadius: StrataRadii.inputRadius,
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(Icons.info_outline, size: 20, color: colors.infoText),
                    const SizedBox(width: StrataSpacing.s2),
                    Expanded(
                      child: Text(
                        l10n.approvalNotice,
                        style: text.bodySmall.copyWith(color: colors.infoText),
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (error != null) ...[
              const SizedBox(height: StrataSpacing.s3),
              FormAlert(message: error),
            ],
            const SizedBox(height: StrataSpacing.s5),
            PrimaryButton(
              label: l10n.requestAccount,
              busy: busy.value,
              onPressed: confirmed ? submit : null,
            ),
            const SizedBox(height: StrataSpacing.s3),
            Wrap(
              alignment: WrapAlignment.center,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  l10n.haveAccount,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
                TextButton(onPressed: back, child: Text(l10n.signInTitle)),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// The core's verdict on a password (`password_strength`): a four-step bar,
/// the level and the length against the server's minimum.
class PasswordStrengthMeter extends ConsumerWidget {
  /// Creates the meter for [password].
  const new({required this.password, super.key});

  /// The password typed so far.
  final String password;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final strength = ref.watch(passwordStrengthProvider(password)).value;
    if (strength == null) return const SizedBox.shrink();
    final (label, color, steps) = switch (strength.level) {
      PasswordLevel.tooShort => (l10n.strengthTooShort, colors.danger, 1),
      PasswordLevel.weak => (l10n.strengthWeak, colors.warning, 2),
      PasswordLevel.fair => (l10n.strengthFair, colors.accent, 3),
      PasswordLevel.strong => (l10n.strengthStrong, colors.success, 4),
    };
    return Semantics(
      container: true,
      liveRegion: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          ExcludeSemantics(
            child: Row(
              children: [
                for (var i = 0; i < 4; i++) ...[
                  if (i > 0) const SizedBox(width: StrataSpacing.s1),
                  Expanded(
                    child: Container(
                      height: 4,
                      decoration: BoxDecoration(
                        color: i < steps ? color : colors.surface2,
                        borderRadius: StrataRadii.pillRadius,
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: StrataSpacing.s1),
          Wrap(
            spacing: StrataSpacing.s1,
            children: [
              Text(
                label,
                style: text.caption
                    .withWeight(FontWeight.w600)
                    .copyWith(color: colors.text2),
              ),
              Text(
                l10n.strengthDetail(
                  length: strength.length,
                  min: strength.minLength,
                ),
                style: text.caption.copyWith(color: colors.text2),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
