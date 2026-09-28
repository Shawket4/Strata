import 'package:file_selector/file_selector.dart' as fs;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/src/boot/file_picker.dart';
import 'package:strata_state/strata_state.dart' show PickedFileType;

void main() {
  group('FileSelectorPicker', () {
    test('save asks the native dialog with the type and name', () async {
      final asked = <(List<fs.XTypeGroup>, String?, bool?)>[];
      final picker = FileSelectorPicker(
        saveDialog:
            ({
              acceptedTypeGroups = const [],
              initialDirectory,
              suggestedName,
              confirmButtonText,
              canCreateDirectories,
            }) async {
              asked.add((
                acceptedTypeGroups,
                suggestedName,
                canCreateDirectories,
              ));
              return const fs.FileSaveLocation('/home/shawket/vault.zip');
            },
      );
      expect(
        await picker.saveFile(
          suggestedName: 'strata-vault.zip',
          type: PickedFileType.zip,
        ),
        '/home/shawket/vault.zip',
      );
      final (groups, name, create) = asked.single;
      final group = groups.single;
      expect(group.label, 'Zip');
      expect(group.extensions, ['zip']);
      expect(group.mimeTypes, ['application/zip']);
      expect((name, create), ('strata-vault.zip', true));
    });

    test('a cancelled dialog is null', () async {
      final picker = FileSelectorPicker(
        saveDialog: ({
          acceptedTypeGroups = const [],
          initialDirectory,
          suggestedName,
          confirmButtonText,
          canCreateDirectories,
        }) async => null,
        openDialog: ({
          acceptedTypeGroups = const [],
          initialDirectory,
          confirmButtonText,
        }) async => null,
      );
      expect(
        await picker.saveFile(
          suggestedName: 'x.md',
          type: PickedFileType.markdown,
        ),
        isNull,
      );
      expect(await picker.openFile(type: PickedFileType.zip), isNull);
    });

    test('open returns the chosen file of the type', () async {
      final asked = <List<fs.XTypeGroup>>[];
      final picker = FileSelectorPicker(
        openDialog:
            ({
              acceptedTypeGroups = const [],
              initialDirectory,
              confirmButtonText,
            }) async {
              asked.add(acceptedTypeGroups);
              return fs.XFile('/home/shawket/notes.zip');
            },
      );
      expect(
        await picker.openFile(type: PickedFileType.zip),
        '/home/shawket/notes.zip',
      );
      expect(asked.single.single.extensions, ['zip']);
    });
  });
}
