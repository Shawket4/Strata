/// Strata inbox feature (UI only; view-models come from the Rust core,
/// PLAN L15).
library;

import 'package:flutter/material.dart';
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// Entry widget of the inbox feature. Inbox of captures with AI filing
/// suggestions (PLAN §11 screen 3).
///
/// Until the Rust core streams this screen's view-model it renders the
/// design-system placeholder state.
class InboxScreen extends StatelessWidget {
  /// Creates the inbox entry widget.
  const new({super.key});

  /// The icon that represents this feature.
  static const IconData icon = Icons.move_to_inbox_outlined;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return StrataEmptyState(
      icon: icon,
      title: l10n.navInbox,
      message: l10n.featurePlaceholderMessage,
    );
  }
}
