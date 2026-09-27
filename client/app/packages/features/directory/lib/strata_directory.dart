/// Strata directory feature (UI only; view-models come from the Rust core,
/// PLAN L15).
library;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the directory feature. Directory: people, companies,
/// documents and places (PLAN §11 screen 7).
///
/// Until the Rust core streams this screen's view-model it renders the
/// design-system placeholder state.
class DirectoryScreen extends StatelessWidget {
  /// Creates the directory entry widget.
  const new({super.key});

  /// The icon that represents this feature.
  static const IconData icon = Icons.people_outline;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return StrataEmptyState(
      icon: icon,
      title: l10n.navDirectory,
      message: l10n.featurePlaceholderMessage,
    );
  }
}
