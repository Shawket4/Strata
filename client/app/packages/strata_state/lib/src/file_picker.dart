/// The seam between the feature UIs and the platform's native file dialogs
/// (owner decision 2026-09-28: `file_selector`, allowed in the app shell
/// only). A picker only returns a path; the core reads or writes the file
/// (`export_vault`, `import_vault`, `download_export`, `export_unsynced`).
library;

import 'package:flutter/foundation.dart' show immutable;

/// A kind of file the dialogs filter for.
@immutable
final class PickedFileType {
  /// Creates a file type [label]led, matching [extensions] (without dots)
  /// and [mimeTypes].
  const new({
    required this.label,
    required this.extensions,
    this.mimeTypes = const [],
  });

  /// A zip archive (vault exports and imports).
  static const PickedFileType zip = PickedFileType(
    label: 'Zip',
    extensions: ['zip'],
    mimeTypes: ['application/zip'],
  );

  /// A Markdown file (unsynced changes).
  static const PickedFileType markdown = PickedFileType(
    label: 'Markdown',
    extensions: ['md'],
    mimeTypes: ['text/markdown'],
  );

  /// Shown name of the filter.
  final String label;

  /// File extensions, without the dot.
  final List<String> extensions;

  /// MIME types (the platforms that filter by type).
  final List<String> mimeTypes;
}

/// The OS file dialogs, as the app shell provides them.
abstract interface class FilePicker {
  /// Asks where to save a file of [type], proposing [suggestedName]; the
  /// chosen path, or `null` when the user cancelled.
  Future<String?> saveFile({
    required String suggestedName,
    required PickedFileType type,
  });

  /// Asks for an existing file of [type]; its path, or `null` when the user
  /// cancelled.
  Future<String?> openFile({required PickedFileType type});
}
