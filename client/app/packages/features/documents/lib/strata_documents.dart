/// Strata documents feature (UI only; view-models come from the Rust core,
/// PLAN L15).
library;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the documents feature. Documents and places pages (PLAN §11
/// screens 7a, 7b).
///
/// Until the Rust core streams this screen's view-model it renders the
/// design-system placeholder state.
class DocumentsScreen extends StatelessWidget {
  /// Creates the documents entry widget.
  const new({super.key});

  /// The icon that represents this feature.
  static const IconData icon = Icons.folder_copy_outlined;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return StrataEmptyState(
      icon: icon,
      title: l10n.featureDocuments,
      message: l10n.featurePlaceholderMessage,
    );
  }
}
