import 'package:flutter/material.dart';
import 'package:strata_ask/src/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';



/// The loading state.
class AskLoading extends StatelessWidget {
  /// Creates the loading state.
  const new({super.key});

  @override
  Widget build(BuildContext context) => Center(
    child: Semantics(
      label: context.askL10n.loading,
      child: const SizedBox.square(
        dimension: 32,
        child: CircularProgressIndicator(strokeWidth: 3),
      ),
    ),
  );
}

/// The error state: the core's failure code.
class AskError extends StatelessWidget {
  /// Creates the error state for [error].
  const new({required this.error, super.key});

  /// The core's error.
  final Object error;

  @override
  Widget build(BuildContext context) {
    final l10n = context.askL10n;
    final failure = error;
    return StrataEmptyState(
      icon: Icons.error_outline,
      title: l10n.errorTitle,
      message: l10n.errorMessage(
        code: failure is CoreFailure ? failure.code : 'unknown',
      ),
    );
  }
}
