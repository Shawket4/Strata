/// Source-level comparison of the flutter_rust_bridge facade with `CoreApi`.
///
/// The generated api files are `dart format`ted flutter_rust_bridge output
/// with a fixed shape (top-level `Future<T>` / `Stream<T>` functions with
/// named parameters), and `CoreApi` is formatted Dart in this package, so a
/// declaration parser over normalised source is exact for both. Every
/// declaration start is also counted separately, so a declaration the parser
/// cannot read fails loudly instead of being skipped.
library;

import 'dart:io';
import 'dart:isolate';

/// One function: return type, name and normalised parameter list.
final class ApiFunction {
  /// Creates a parsed function.
  const new(this.returnType, this.name, this.parameters);

  /// `Future<SessionState>`, `Stream<HomeView>`, …
  final String returnType;

  /// Function name.
  final String name;

  /// Parameters as written, normalised (`{required String id}`), `''` when
  /// there are none.
  final String parameters;

  /// Canonical signature, e.g. `Stream<NoteScreen> watchNote({required
  /// String id})`.
  String get signature => '$returnType $name($parameters)';

  @override
  String toString() => signature;
}

/// Thrown when a source does not have the expected shape.
final class SurfaceParseError extends Error {
  /// Creates the error.
  new(this.message);

  /// What went wrong.
  final String message;

  @override
  String toString() => 'SurfaceParseError: $message';
}

const _nonFunctionStarts = {'import', 'export', 'part', 'library'};

String _stripComments(String source) => source
    .split('\n')
    .map((line) => line.trimLeft().startsWith('//') ? '' : line)
    .join('\n');

String _normaliseParameters(String raw) {
  var text = raw.replaceAll(RegExp(r'\s+'), ' ').trim();
  text = text.replaceAll(RegExp(r',\s*}'), '}').replaceAll(RegExp(r',$'), '');
  text = text.replaceAll('{ ', '{').replaceAll(' }', '}');
  return text;
}

Map<String, ApiFunction> _collect(Iterable<RegExpMatch> matches, String where) {
  final functions = <String, ApiFunction>{};
  for (final m in matches) {
    final function = ApiFunction(
      m.group(1)!.replaceAll(RegExp(r'\s+'), ' ').trim(),
      m.group(2)!,
      _normaliseParameters(m.group(3)!),
    );
    if (functions.containsKey(function.name)) {
      throw SurfaceParseError('$where declares ${function.name} twice');
    }
    functions[function.name] = function;
  }
  return functions;
}

/// Parses the top-level functions of one generated api file.
Map<String, ApiFunction> parseFacadeFile(String source, {String where = ''}) {
  final code = _stripComments(source);
  final matches = RegExp(
    r'^([A-Za-z][\w<>?, ]*?)\s+(\w+)\(([^)]*)\)\s*(?:=>|\{|async)',
    multiLine: true,
  ).allMatches(code).toList();
  final starts = RegExp(r'^([A-Za-z]\w*)', multiLine: true)
      .allMatches(code)
      .where((m) => !_nonFunctionStarts.contains(m.group(1)))
      .length;
  if (starts != matches.length) {
    throw SurfaceParseError(
      '$where: $starts top-level declarations but ${matches.length} parsed '
      'functions; update the parser for the new shape',
    );
  }
  return _collect(matches, where);
}

/// Parses the abstract methods of `abstract interface class [className]`.
Map<String, ApiFunction> parseInterface(
  String source, {
  String className = 'CoreApi',
}) {
  final code = _stripComments(source);
  final header = RegExp('abstract interface class $className\\s*\\{')
      .firstMatch(code);
  if (header == null) throw SurfaceParseError('no interface $className');
  final end = code.indexOf('\n}', header.end);
  final body = code.substring(header.end, end);
  final matches = RegExp(
    r'^  ([A-Za-z][\w<>?, ]*?)\s+(\w+)\(([^)]*)\);',
    multiLine: true,
  ).allMatches(body).toList();
  final starts = RegExp(r'^  [A-Za-z]', multiLine: true).allMatches(body);
  if (starts.length != matches.length) {
    throw SurfaceParseError(
      '$className: ${starts.length} members but ${matches.length} parsed '
      'abstract methods',
    );
  }
  return _collect(matches, className);
}

/// Differences between the facade and the interface, as readable lines
/// (empty when they match 1:1).
List<String> diffSurfaces(
  Map<String, ApiFunction> facade,
  Map<String, ApiFunction> interface,
) {
  final names = {...facade.keys, ...interface.keys}.toList()..sort();
  return [
    for (final name in names)
      if (!interface.containsKey(name))
        'missing in CoreApi: ${facade[name]}'
      else if (!facade.containsKey(name))
        'not in the facade: ${interface[name]}'
      else if (facade[name]!.signature != interface[name]!.signature)
        'signature differs: facade ${facade[name]} vs CoreApi '
            '${interface[name]}',
  ];
}

/// Checks that every `BridgeCoreApi` method forwards to the generated
/// function of the same name with each parameter passed through unchanged.
List<String> checkDelegation(
  String bridgeSource,
  Map<String, ApiFunction> facade,
) {
  final code = _stripComments(bridgeSource);
  final forwards = RegExp(r'(\w+)\(([^)]*)\)\s*=>\s*bridge\.(\w+)\(([^)]*)\)')
      .allMatches(code);
  final problems = <String>[];
  final seen = <String>{};
  for (final m in forwards) {
    final method = m.group(1)!;
    final target = m.group(3)!;
    seen.add(method);
    if (method != target) {
      problems.add('$method forwards to bridge.$target');
      continue;
    }
    final names = RegExp(r'(\w+)\s*(?:,|$|})')
        .allMatches(_normaliseParameters(m.group(2)!))
        .map((p) => p.group(1));
    final expected = names.map((n) => '$n: $n').join(', ');
    final actual = _normaliseParameters(m.group(4)!);
    if (expected != actual) {
      problems.add('$method passes ($actual), expected ($expected)');
    }
  }
  for (final name in facade.keys) {
    if (!seen.contains(name)) problems.add('BridgeCoreApi lacks $name');
  }
  return problems;
}

/// The directory of [packageUri]'s `lib/`, resolved through the package
/// config (independent of the working directory).
Future<Directory> packageLib(String package) async {
  final uri = await Isolate.resolvePackageUri(
    Uri.parse('package:$package/$package.dart'),
  );
  if (uri == null) throw StateError('package $package not resolved');
  return File.fromUri(uri).parent;
}

/// The generated facade files of `strata_bridge`.
Future<List<File>> facadeFiles() async {
  final api = Directory(
    '${(await packageLib('strata_bridge')).path}'
    '/src/generated/api',
  );
  return api
      .listSync()
      .whereType<File>()
      .where((f) => f.path.endsWith('.dart'))
      .toList()
    ..sort((a, b) => a.path.compareTo(b.path));
}

/// Every function of the generated facade.
Future<Map<String, ApiFunction>> loadFacade() async {
  final facade = <String, ApiFunction>{};
  for (final file in await facadeFiles()) {
    final parsed = parseFacadeFile(file.readAsStringSync(), where: file.path);
    for (final entry in parsed.entries) {
      if (facade.containsKey(entry.key)) {
        throw SurfaceParseError('${entry.key} is declared in two api files');
      }
      facade[entry.key] = entry.value;
    }
  }
  return facade;
}
