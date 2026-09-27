import 'package:flutter/material.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// A screen with one button opening the account sheet.
class AccountHost extends StatelessWidget {
  /// Creates the host.
  const AccountHost({super.key, this.onDevices, this.onAdmin});

  /// Devices tapped.
  final VoidCallback? onDevices;

  /// Admin → Users tapped.
  final VoidCallback? onAdmin;

  /// The open button's label.
  static const String openLabel = 'Open account';

  @override
  Widget build(BuildContext context) => Scaffold(
    body: Center(
      child: Builder(
        builder: (context) => FilledButton(
          onPressed: () => showAccountSheet(
            context,
            onOpenDevices: onDevices,
            onOpenAdminUsers: onAdmin,
          ),
          child: const Text(openLabel),
        ),
      ),
    ),
  );
}

/// The account sheet (compact) or dialog (wider) drawn in place, for goldens.
class AccountSheetPreview extends StatelessWidget {
  /// Creates the preview.
  const AccountSheetPreview({required this.sizeClass, super.key});

  /// Which surface.
  final SizeClass sizeClass;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Scaffold(
      body: ColoredBox(
        color: colors.scrim,
        child: sizeClass == SizeClass.compact
            ? Align(
                alignment: Alignment.bottomCenter,
                child: Material(
                  color: colors.surface,
                  borderRadius: StrataRadii.sheetTopRadius,
                  child: const Padding(
                    padding: EdgeInsets.only(top: 24),
                    child: AccountSheet(),
                  ),
                ),
              )
            : Dialog(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 420),
                  child: AccountSheet(),
                ),
              ),
      ),
    );
  }
}
