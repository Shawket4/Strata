import 'package:flutter/material.dart';
import 'package:strata_accounts/src/l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// The frame of the signed-out screens (SCREEN_SPEC LoginCompact /
/// LoginExpanded): on compact one scrolling column over the strata bands
/// (without the sand seam on phones, owner decision 2026-09-28); on medium a
/// centred form over the bands with the seam; on expanded a split with the
/// brand panel. On compact and medium the bands run to the bottom edge,
/// under the system navigation inset, and keep their full depth above it.
class AuthLayout extends StatelessWidget {
  /// Creates the frame around [child] (the form column).
  const new({required this.child, super.key, this.header});

  /// The form and its links.
  final Widget child;

  /// Optional header row above the form on compact (e.g. a back button).
  final Widget? header;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final sizeClass = SizeClass.of(context);
    final head = header;
    // The bottom inset (gesture or navigation bar) the bands extend under.
    final inset = MediaQuery.paddingOf(context).bottom;
    Widget bands({required double height, required bool seam}) =>
        ExcludeSemantics(
          child: SizedBox(
            height: height + inset,
            child: StrataBands(showSeam: seam, bleed: inset),
          ),
        );
    Widget form(double maxWidth) => Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s6,
          vertical: StrataSpacing.s8,
        ),
        child: ConstrainedBox(
          constraints: BoxConstraints(maxWidth: maxWidth),
          child: child,
        ),
      ),
    );
    return Scaffold(
      backgroundColor: colors.background,
      body: SafeArea(
        // The bands of compact and medium fill the bottom inset themselves.
        bottom: sizeClass == SizeClass.expanded,
        child: switch (sizeClass) {
          SizeClass.compact => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ?head,
              Expanded(
                child: SingleChildScrollView(
                  padding: EdgeInsets.fromLTRB(
                    StrataSpacing.s6,
                    head == null ? StrataSpacing.s10 : StrataSpacing.s2,
                    StrataSpacing.s6,
                    StrataSpacing.s6,
                  ),
                  child: child,
                ),
              ),
              bands(height: 72, seam: false),
            ],
          ),
          SizeClass.medium => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ?head,
              Expanded(child: form(440)),
              bands(height: 96, seam: true),
            ],
          ),
          SizeClass.expanded => Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Expanded(flex: 43, child: BrandPanel()),
              Expanded(
                flex: 57,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    ?head,
                    Expanded(child: form(420)),
                  ],
                ),
              ),
            ],
          ),
        },
      ),
    );
  }
}

/// The brand panel of the expanded sign-in (wordmark, tagline, bands).
class BrandPanel extends StatelessWidget {
  /// Creates the panel.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.accountsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return ColoredBox(
      color: colors.surface2,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(StrataSpacing.s12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.brandKicker,
                    style: text.caption
                        .withWeight(FontWeight.w600)
                        .copyWith(color: colors.text2, letterSpacing: 1.2),
                  ),
                  const SizedBox(height: StrataSpacing.s12),
                  const StrataWordmark(fontSize: 60),
                  const SizedBox(height: StrataSpacing.s4),
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 440),
                    child: Text(
                      l10n.brandTagline,
                      style: text.body.copyWith(
                        fontSize: StrataTypeScale.titleSmall,
                        color: colors.text2,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
          const ExcludeSemantics(
            child: SizedBox(height: 240, child: StrataBands()),
          ),
        ],
      ),
    );
  }
}

/// A labelled form field as in the artboards: bold label above the input.
class LabeledField extends StatelessWidget {
  /// Creates the field.
  const new({
    required this.label,
    required this.controller,
    super.key,
    this.helper,
    this.error,
    this.obscure = false,
    this.mono = false,
    this.keyboardType,
    this.textInputAction = TextInputAction.next,
    this.onSubmitted,
    this.suffix,
    this.autofillHints,
    this.prefixText,
    this.enabled = true,
  });

  /// Visible and semantic label.
  final String label;

  /// Text controller (ephemeral widget state).
  final TextEditingController controller;

  /// Helper line.
  final String? helper;

  /// Error line.
  final String? error;

  /// Obscures the text (passwords).
  final bool obscure;

  /// Uses the monospace style (URLs, usernames).
  final bool mono;

  /// Keyboard type.
  final TextInputType? keyboardType;

  /// Keyboard action.
  final TextInputAction textInputAction;

  /// Called on the keyboard action.
  final ValueChanged<String>? onSubmitted;

  /// Trailing widget (e.g. show password).
  final Widget? suffix;

  /// Autofill hints.
  final Iterable<String>? autofillHints;

  /// Prefix inside the field (e.g. `@`).
  final String? prefixText;

  /// Whether the field is enabled.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final text = context.strataText;
    return TextField(
      controller: controller,
      obscureText: obscure,
      enabled: enabled,
      keyboardType: keyboardType,
      textInputAction: textInputAction,
      onSubmitted: onSubmitted,
      autofillHints: autofillHints,
      autocorrect: false,
      enableSuggestions: !obscure,
      style: mono ? text.mono : text.body,
      decoration: InputDecoration(
        labelText: label,
        floatingLabelBehavior: FloatingLabelBehavior.always,
        helperText: helper,
        helperMaxLines: 3,
        errorText: error,
        errorMaxLines: 3,
        suffixIcon: suffix,
        prefixText: prefixText,
      ),
    );
  }
}

/// The show/hide password toggle.
class PasswordVisibilityButton extends StatelessWidget {
  /// Creates the toggle.
  const new({required this.obscured, required this.onToggle, super.key});

  /// Whether the password is hidden.
  final bool obscured;

  /// Toggles visibility.
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final l10n = context.accountsL10n;
    return IconButton(
      tooltip: obscured ? l10n.showPassword : l10n.hidePassword,
      onPressed: onToggle,
      icon: Icon(
        obscured ? Icons.visibility_outlined : Icons.visibility_off_outlined,
      ),
    );
  }
}

/// An inline alert (role alert) for a failed intent.
class FormAlert extends StatelessWidget {
  /// Creates the alert with [message].
  const new({required this.message, super.key});

  /// The localised failure.
  final String message;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      liveRegion: true,
      container: true,
      child: Container(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        decoration: BoxDecoration(
          color: colors.dangerTint,
          borderRadius: StrataRadii.inputRadius,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.error_outline, size: 20, color: colors.dangerText),
            const SizedBox(width: StrataSpacing.s2),
            Expanded(
              child: Text(
                message,
                style: text.bodySmall.copyWith(color: colors.dangerText),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A full-width primary button with a busy state.
class PrimaryButton extends StatelessWidget {
  /// Creates the button.
  const new({
    required this.label,
    required this.onPressed,
    super.key,
    this.busy = false,
    this.icon,
    this.danger = false,
  });

  /// Label.
  final String label;

  /// Action; `null` disables the button.
  final VoidCallback? onPressed;

  /// Shows a spinner and disables the button.
  final bool busy;

  /// Optional leading icon.
  final IconData? icon;

  /// Uses the danger colour.
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final lead = icon;
    return FilledButton(
      onPressed: busy ? null : onPressed,
      style: FilledButton.styleFrom(
        minimumSize: const Size.fromHeight(StrataLayout.minTouchTarget),
        backgroundColor: danger ? colors.dangerTint : null,
        foregroundColor: danger ? colors.dangerText : null,
      ),
      child: busy
          ? SizedBox.square(
              dimension: 20,
              child: CircularProgressIndicator(
                strokeWidth: 2,
                color: colors.text2,
              ),
            )
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (lead != null) ...[
                  Icon(lead, size: 20),
                  const SizedBox(width: StrataSpacing.s2),
                ],
                Flexible(child: Text(label, textAlign: TextAlign.center)),
              ],
            ),
    );
  }
}
