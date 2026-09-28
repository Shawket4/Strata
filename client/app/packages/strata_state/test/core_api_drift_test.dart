// Guards the 1:1 mirror between the generated flutter_rust_bridge facade
// (strata_bridge/lib/src/generated/api/*.dart) and CoreApi. It runs in
// `melos run test`; after `melos run gen:bridge` adds, removes or changes a
// facade function, this test names the CoreApi / BridgeCoreApi / FakeCoreApi /
// strata_state.dart edits to make. (BridgeCoreApi and FakeCoreApi implement
// CoreApi, so the compiler already checks them against the interface.)
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/api_surface.dart';

void main() {
  late Map<String, ApiFunction> facade;
  late String interfaceSource;
  late String bridgeSource;
  late String librarySource;
  late String modelSource;
  late String fixturesSource;

  setUpAll(() async {
    facade = await loadFacade();
    final lib = packageLib('strata_state').path;
    final bridgeLib = packageLib('strata_bridge').path;
    interfaceSource = File('$lib/src/core_api.dart').readAsStringSync();
    bridgeSource = File('$lib/src/bridge_core_api.dart').readAsStringSync();
    librarySource = File('$lib/strata_state.dart').readAsStringSync();
    fixturesSource = File('$lib/src/testing/fixtures.dart').readAsStringSync();
    modelSource = File('$bridgeLib/src/generated/view/model.dart')
        .readAsStringSync();
  });

  group('CoreApi mirrors the generated facade', () {
    test('reads all four api files and every function in them', () async {
      final files = (await facadeFiles())
          .map((f) => f.uri.pathSegments.last)
          .toList();
      expect(files, [
        'app.dart',
        'intents.dart',
        'reminders.dart',
        'views.dart',
      ]);
      expect(facade, hasLength(143));
      expect(
        facade['watchDirectory']!.signature,
        'Stream<DirectoryView> watchDirectory({required DirectoryTab tab, '
        'required String query})',
      );
      expect(
        facade['editorHints']!.signature,
        'Future<List<EditorHint>> editorHints({required String content})',
      );
      expect(facade['syncNow']!.signature, 'Future<void> syncNow()');
    });

    test('has exactly the same functions and signatures', () {
      final interface = parseInterface(interfaceSource);
      expect(diffSurfaces(facade, interface), isEmpty);
      expect(interface.keys.toSet(), facade.keys.toSet());
    });

    test('BridgeCoreApi forwards each method to its namesake unchanged', () {
      expect(checkDelegation(bridgeSource, facade), isEmpty);
    });

    test('strata_state.dart hides every facade function from its export', () {
      final hide = RegExp(r'hide\s+([^;]+);')
          .firstMatch(librarySource)!
          .group(1)!
          .split(',')
          .map((s) => s.trim());
      expect(hide.toSet(), containsAll(facade.keys));
    });

    test('StrataFixtures has a fixture for every view-model class', () {
      final classes = RegExp(
        r'^class (\w+)',
        multiLine: true,
      ).allMatches(modelSource).map((m) => m.group(1)!).toSet();
      final fixtureTypes = RegExp(
        r'static (?:const|final) (?:List<)?(\w+)>? \w+ =',
      ).allMatches(fixturesSource).map((m) => m.group(1)!).toSet();
      expect(classes, hasLength(131));
      expect(classes.difference(fixtureTypes), isEmpty);
    });
  });

  group('the drift check catches', () {
    test('a facade function missing from CoreApi', () {
      final withoutWatchHome = interfaceSource.replaceFirst(
        RegExp(r'\n  /// Home: [^\n]*\n  Stream<HomeView> watchHome\(\);\n'),
        '\n',
      );
      expect(withoutWatchHome, isNot(interfaceSource));
      expect(diffSurfaces(facade, parseInterface(withoutWatchHome)), [
        'missing in CoreApi: Stream<HomeView> watchHome()',
      ]);
    });

    test('a new facade function', () {
      final grown = {
        ...facade,
        ...parseFacadeFile('''
/// Archives a note.
Future<String> archiveNote({required String id}) =>
    StrataCore.instance.api.crateApiIntentsArchiveNote(id: id);
'''),
      };
      expect(diffSurfaces(grown, parseInterface(interfaceSource)), [
        'missing in CoreApi: Future<String> archiveNote({required String id})',
      ]);
    });

    test('a changed parameter or return type', () {
      final changed = interfaceSource
          .replaceFirst(
            'Future<String> capture({required String text});',
            'Future<void> capture({required String text});',
          )
          .replaceFirst(
            'Stream<NoteScreen> watchNote({required String id});',
            'Stream<NoteScreen> watchNote({required int id});',
          );
      const capture =
          'signature differs: '
          'facade Future<String> capture({required String text}) vs '
          'CoreApi Future<void> capture({required String text})';
      const watchNote =
          'signature differs: '
          'facade Stream<NoteScreen> watchNote({required String id}) vs '
          'CoreApi Stream<NoteScreen> watchNote({required int id})';
      expect(diffSurfaces(facade, parseInterface(changed)), [
        capture,
        watchNote,
      ]);
    });

    test('an extra CoreApi method the facade does not have', () {
      final extra = interfaceSource.replaceFirst(
        'abstract interface class CoreApi {',
        'abstract interface class CoreApi {\n'
            '  Future<void> legacyReset();\n',
      );
      expect(diffSurfaces(facade, parseInterface(extra)), [
        'not in the facade: Future<void> legacyReset()',
      ]);
    });

    test('a wrong delegation in BridgeCoreApi', () {
      final wrong = bridgeSource
          .replaceFirst('bridge.watchInbox()', 'bridge.watchHome()')
          .replaceFirst(
            'bridge.moveNote(id: id, newPath: newPath)',
            'bridge.moveNote(id: newPath, newPath: id)',
          );
      const moveNote =
          'moveNote passes (id: newPath, newPath: id), '
          'expected (id: id, newPath: newPath)';
      expect(checkDelegation(wrong, facade), [
        moveNote,
        'watchInbox forwards to bridge.watchHome',
      ]);
    });

    test('a facade declaration the parser cannot read', () {
      expect(
        () => parseFacadeFile('int get coreVersion => 3;\n'),
        throwsA(isA<SurfaceParseError>()),
      );
    });
  });
}
