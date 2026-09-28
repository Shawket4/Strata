import 'package:file_selector/file_selector.dart' as fs;
import 'package:strata_state/strata_state.dart' show FilePicker, PickedFileType;

/// Shows the platform's save dialog (`file_selector`'s `getSaveLocation`).
typedef SaveDialog = Future<fs.FileSaveLocation?> Function({
  List<fs.XTypeGroup> acceptedTypeGroups,
  String? initialDirectory,
  String? suggestedName,
  String? confirmButtonText,
  bool? canCreateDirectories,
});

/// Shows the platform's open dialog (`file_selector`'s `openFile`).
typedef OpenDialog = Future<fs.XFile?> Function({
  List<fs.XTypeGroup> acceptedTypeGroups,
  String? initialDirectory,
  String? confirmButtonText,
});

/// The native file dialogs of every platform (`file_selector`, allowed in
/// the app shell only; owner decision 2026-09-28). It only returns the path
/// the user chose; the Rust core reads or writes the file.
final class FileSelectorPicker implements FilePicker {
  /// Creates the picker; tests pass their own dialogs.
  const new({
    SaveDialog saveDialog = fs.getSaveLocation,
    OpenDialog openDialog = fs.openFile,
  }) : _save = saveDialog,
       _open = openDialog;

  final SaveDialog _save;
  final OpenDialog _open;

  static fs.XTypeGroup _group(PickedFileType type) => fs.XTypeGroup(
    label: type.label,
    extensions: type.extensions,
    mimeTypes: type.mimeTypes,
  );

  @override
  Future<String?> saveFile({
    required String suggestedName,
    required PickedFileType type,
  }) async => (await _save(
    acceptedTypeGroups: [_group(type)],
    suggestedName: suggestedName,
    canCreateDirectories: true,
  ))?.path;

  @override
  Future<String?> openFile({required PickedFileType type}) async =>
      (await _open(acceptedTypeGroups: [_group(type)]))?.path;
}
