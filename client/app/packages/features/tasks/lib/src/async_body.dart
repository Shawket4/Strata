import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/src/l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Renders one core view-model stream: [data] once a value arrived, a
/// labelled progress indicator while loading, and the core's failure (final;
/// the core owns retries, §12.4) otherwise.
class CoreAsyncBody<T> extends StatelessWidget {
  /// Creates the body for [value].
  const new({
    required this.value,
    required this.data,
    required this.errorTitle,
    super.key,
  });

  /// The provider's current value.
  final AsyncValue<T> value;

  /// Builds the content.
  final Widget Function(T value) data;

  /// Headline of the error state (e.g. "Couldn't load tasks").
  final String errorTitle;

  @override
  Widget build(BuildContext context) {
    return switch (value) {
      AsyncData(:final value) => data(value),
      AsyncError(:final error) => CoreErrorState(
        title: errorTitle,
        error: error,
      ),
      _ => const CoreLoadingState(),
    };
  }
}

/// A centred, labelled progress indicator.
class CoreLoadingState extends StatelessWidget {
  /// Creates the loading state.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    return TasksL10nScope(
      child: Builder(
        builder: (context) => Center(
          child: Semantics(
            label: context.tasksL10n.commonLoading,
            child: const SizedBox.square(
              dimension: 32,
              child: CircularProgressIndicator(),
            ),
          ),
        ),
      ),
    );
  }
}

/// The failure of a core stream or intent: [title] and the core's
/// localisation key / code (a [CoreFailure]), or a generic message.
class CoreErrorState extends StatelessWidget {
  /// Creates the error state.
  const new({required this.title, required this.error, super.key});

  /// Headline.
  final String title;

  /// The error the core reported.
  final Object error;

  @override
  Widget build(BuildContext context) {
    return TasksL10nScope(
      child: Builder(
        builder: (context) {
          final l10n = context.tasksL10n;
          final failure = error;
          return StrataEmptyState(
            icon: Icons.error_outline,
            title: title,
            message: failure is CoreFailure
                ? l10n.commonCoreFailure(code: failure.code)
                : l10n.commonUnknownFailure,
          );
        },
      ),
    );
  }
}
