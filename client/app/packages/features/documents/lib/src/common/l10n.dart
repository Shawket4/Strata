import 'package:flutter/widgets.dart';
import 'package:strata_documents/src/generated/documents_localizations.dart';

export 'package:strata_documents/src/generated/documents_localizations.dart';

/// Access to the documents feature's strings.
extension DocumentsL10nContext on BuildContext {
  /// The [DocumentsLocalizations] of the ambient locale.
  DocumentsLocalizations get docsL10n => DocumentsLocalizations.of(this);
}

/// Adds the documents feature's localizations delegate below the app's
/// [Localizations] (same locale), so entry widgets work under any app shell.
class DocumentsLocalizationScope extends StatelessWidget {
  /// Wraps [child].
  const new({required this.child, super.key});

  /// The feature UI.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (Localizations.of<DocumentsLocalizations>(
          context,
          DocumentsLocalizations,
        ) !=
        null) {
      return child;
    }
    return Localizations.override(
      context: context,
      delegates: const [DocumentsLocalizations.delegate],
      child: child,
    );
  }
}
