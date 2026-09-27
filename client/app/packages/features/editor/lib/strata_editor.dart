/// Strata editor feature (UI only; view-models come from the Rust core,
/// PLAN L15).
library;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the editor feature. Note view/editor with properties,
/// backlinks and history (PLAN §11 screen 4).
///
/// Until the Rust core streams this screen's view-model it renders the
/// design-system placeholder state.
class NoteEditorScreen extends StatelessWidget {
  /// Creates the editor entry widget.
  const new({super.key});

  /// The icon that represents this feature.
  static const IconData icon = Icons.edit_note;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return StrataEmptyState(
      icon: icon,
      title: l10n.featureEditor,
      message: l10n.featurePlaceholderMessage,
    );
  }
}
