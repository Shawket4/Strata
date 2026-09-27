import 'package:flutter/material.dart';
import 'package:strata_maps/src/common/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// The loading state of a maps screen.
class MapsLoading extends StatelessWidget {
  /// Creates the loading state.
  const new({super.key});

  @override
  Widget build(BuildContext context) => Center(
    child: Semantics(
      label: context.mapsL10n.loading,
      child: const SizedBox.square(
        dimension: 32,
        child: CircularProgressIndicator(strokeWidth: 3),
      ),
    ),
  );
}

/// The error state of a maps screen: the core's failure code, rendered.
class MapsError extends StatelessWidget {
  /// Creates the error state for [error].
  const new({required this.error, super.key});

  /// The error from the core (a `CoreFailure` when typed).
  final Object error;

  @override
  Widget build(BuildContext context) {
    final l10n = context.mapsL10n;
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
