import 'dart:convert';
import 'dart:io';

import 'package:test/test.dart';

/// Arrow glyphs banned from user-visible text (docs/DECISIONS.md): Cairo has
/// no arrow glyph, and a fixed arrow points the wrong way in RTL.
const _arrows = ['→', '←', '⇒', '➜'];

/// Every ARB file under `apps/` and `packages/`, sorted by path.
List<File> _arbFiles(Directory root) {
  final files = <File>[];
  for (final dir in ['apps', 'packages']) {
    final base = Directory('${root.path}/$dir');
    if (!base.existsSync()) continue;
    for (final entity in base.listSync(recursive: true)) {
      final path = entity.path;
      if (entity is File &&
          path.endsWith('.arb') &&
          !path.contains('/build/') &&
          !path.contains('/.dart_tool/')) {
        files.add(entity);
      }
    }
  }
  return files..sort((a, b) => a.path.compareTo(b.path));
}

/// `file: key: value` for every user-visible ARB value holding an arrow.
List<String> _violations(List<File> files) {
  final found = <String>[];
  for (final file in files) {
    final arb = jsonDecode(file.readAsStringSync()) as Map<String, Object?>;
    for (final MapEntry(:key, :value) in arb.entries) {
      if (key.startsWith('@') || value is! String) continue;
      if (_arrows.any(value.contains)) {
        found.add('${file.path}: $key: $value');
      }
    }
  }
  return found;
}

void main() {
  test('no ARB value in the workspace contains an arrow glyph', () {
    final files = _arbFiles(Directory.current);
    expect(files, isNotEmpty, reason: 'run from the workspace root');
    expect(_violations(files), isEmpty);
  });

  test('the check flags every arrow and ignores metadata', () {
    final dir = Directory.systemTemp.createTempSync('arb_arrows_');
    addTearDown(() => dir.deleteSync(recursive: true));
    final file = File('${dir.path}/x_en.arb')
      ..writeAsStringSync(
        jsonEncode({
          '@@locale': 'en',
          'a': 'A → B',
          'b': 'A ← B',
          'c': 'A ⇒ B',
          'd': 'A ➜ B',
          'ok': 'A · B',
          '@a': {'description': 'Opens A → B.'},
        }),
      );
    expect(_violations([file]), [
      '${file.path}: a: A → B',
      '${file.path}: b: A ← B',
      '${file.path}: c: A ⇒ B',
      '${file.path}: d: A ➜ B',
    ]);
  });
}
