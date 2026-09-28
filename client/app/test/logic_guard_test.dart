import 'dart:io';

import 'package:test/test.dart';

import '../tool/logic_guard.dart';

/// Builds a throwaway workspace: root pubspec + members + files.
Directory _workspace({
  Map<String, String> members = const {},
  Map<String, String> files = const {},
  String rootDevDeps = '  melos: ^8.9.0\n',
}) {
  final root = Directory.systemTemp.createTempSync('logic_guard_');
  addTearDown(() => root.deleteSync(recursive: true));
  final memberDirs = members.keys.toList();
  File('${root.path}/pubspec.yaml').writeAsStringSync(
    'name: ws\n'
    'workspace:\n'
    '${memberDirs.map((d) => '  - $d\n').join()}'
    'dev_dependencies:\n$rootDevDeps',
  );
  for (final entry in members.entries) {
    File('${root.path}/${entry.key}/pubspec.yaml')
      ..createSync(recursive: true)
      ..writeAsStringSync(entry.value);
  }
  for (final entry in files.entries) {
    File('${root.path}/${entry.key}')
      ..createSync(recursive: true)
      ..writeAsStringSync(entry.value);
  }
  return root;
}

String _pubspec(
  String name, {
  String deps = '',
  String devDeps = '',
  String extra = '',
}) =>
    'name: $name\n'
    'dependencies:\n  flutter:\n    sdk: flutter\n$deps'
    'dev_dependencies:\n  flutter_test:\n    sdk: flutter\n$devDeps'
    '$extra';

void main() {
  group('dependency allow-list', () {
    test('accepts the §11.1 stack and workspace packages', () {
      final root = _workspace(
        members: {
          'packages/strata_ui': _pubspec('strata_ui'),
          'packages/features/home': _pubspec(
            'strata_home',
            deps:
                '  strata_ui: any\n  hooks_riverpod: ^3.4.3\n'
                '  flutter_hooks: ^0.21.3\n',
            devDeps: '  alchemist: ^0.14.0\n  mocktail: ^1.0.5\n',
          ),
        },
      );
      expect(runGuard(root), isEmpty);
    });

    test('refuses networking and persistence packages with reasons', () {
      final root = _workspace(
        members: {
          'packages/features/inbox': _pubspec(
            'strata_inbox',
            deps:
                '  http: ^1.0.0\n  drift: ^2.0.0\n  shared_preferences: any\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_inbox',
          'dependencies "drift" is not allowed: '
              'persistence belongs to the Rust core (L14, L15)',
        ),
        const Violation(
          'strata_inbox',
          'dependencies "http" is not allowed: '
              'networking belongs to the Rust core (L15)',
        ),
        const Violation(
          'strata_inbox',
          'dependencies "shared_preferences" is not allowed: '
              'settings are stored by the Rust core (L15)',
        ),
      ]);
    });

    test('refuses unknown packages even when not on the deny list', () {
      final root = _workspace(
        members: {
          'packages/features/ask': _pubspec(
            'strata_ask',
            deps: '  collection: ^1.19.0\n',
            devDeps: '  fake_async: ^1.3.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_ask',
          'dependencies "collection" is not allowed: '
              'not in the §11.1 allow-list',
        ),
        const Violation(
          'strata_ask',
          'dev_dependencies "fake_async" is not allowed: '
              'not in the §11.1 allow-list',
        ),
      ]);
    });

    test('scoped packages are allowed only where they belong', () {
      final root = _workspace(
        members: {
          'packages/strata_l10n': _pubspec(
            'strata_l10n',
            deps:
                '  intl: ^0.20.2\n  flutter_localizations:\n    sdk: flutter\n',
          ),
          'packages/strata_bridge': _pubspec(
            'strata_bridge',
            deps: '  flutter_rust_bridge: ^2.0.0\n  path_provider: ^2.1.0\n',
          ),
          'packages/features/maps': _pubspec(
            'strata_maps',
            deps: '  intl: ^0.20.2\n  path_provider: ^2.1.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_maps',
          'dependencies "intl" is not allowed: not in the §11.1 allow-list',
        ),
        const Violation(
          'strata_maps',
          'dependencies "path_provider" is not allowed: '
              'file-system access is only for strata_bridge (L15)',
        ),
      ]);
    });

    test('timezone is allowed in the app shell only', () {
      final root = _workspace(
        members: {
          'apps/strata': _pubspec(
            'strata',
            deps:
                '  flutter_local_notifications: ^22.3.1\n'
                '  timezone: ^0.11.0\n',
          ),
          'packages/features/tasks': _pubspec(
            'strata_tasks',
            deps: '  timezone: ^0.11.0\n',
          ),
          'packages/strata_state': _pubspec(
            'strata_state',
            deps: '  timezone: ^0.11.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_state',
          'dependencies "timezone" is not allowed: '
              'not in the §11.1 allow-list',
        ),
        const Violation(
          'strata_tasks',
          'dependencies "timezone" is not allowed: '
              'not in the §11.1 allow-list',
        ),
      ]);
    });

    test('file_selector is allowed in the app shell only', () {
      final root = _workspace(
        members: {
          'apps/strata': _pubspec('strata', deps: '  file_selector: ^1.1.0\n'),
          'packages/features/settings': _pubspec(
            'strata_settings',
            deps: '  file_selector: ^1.1.0\n',
          ),
          'packages/strata_state': _pubspec(
            'strata_state',
            deps: '  file_selector: ^1.1.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_settings',
          'dependencies "file_selector" is not allowed: '
              'file dialogs belong to the app shell (FilePicker seam)',
        ),
        const Violation(
          'strata_state',
          'dependencies "file_selector" is not allowed: '
              'file dialogs belong to the app shell (FilePicker seam)',
        ),
      ]);
    });

    test('the icon and splash generators are dev dependencies of the app '
        'shell only', () {
      final root = _workspace(
        members: {
          'apps/strata': _pubspec(
            'strata',
            devDeps:
                '  flutter_launcher_icons: ^0.14.4\n'
                '  flutter_native_splash: ^2.4.8\n',
          ),
          'packages/strata_ui': _pubspec(
            'strata_ui',
            devDeps: '  flutter_native_splash: ^2.4.8\n',
          ),
          'packages/features/home': _pubspec(
            'strata_home',
            deps: '  flutter_launcher_icons: ^0.14.4\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_home',
          'dependencies "flutter_launcher_icons" is not allowed: '
              'not in the §11.1 allow-list',
        ),
        const Violation(
          'strata_ui',
          'dev_dependencies "flutter_native_splash" is not allowed: '
              'not in the §11.1 allow-list',
        ),
      ]);
    });

    test('timezone is not an allowed dev dependency', () {
      final root = _workspace(
        members: {
          'apps/strata': _pubspec('strata', devDeps: '  timezone: ^0.11.0\n'),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata',
          'dev_dependencies "timezone" is not allowed: '
              'not in the §11.1 allow-list',
        ),
      ]);
    });

    test('alchemist is a runtime dependency of strata_state only', () {
      final root = _workspace(
        members: {
          'packages/strata_state': _pubspec(
            'strata_state',
            deps: '  alchemist: ^0.14.0\n',
          ),
          'packages/features/home': _pubspec(
            'strata_home',
            deps: '  alchemist: ^0.14.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata_home',
          'dependencies "alchemist" is not allowed: '
              'not in the §11.1 allow-list',
        ),
      ]);
    });

    test('refuses path/git dependencies, unknown SDK packages, overrides', () {
      final root = _workspace(
        members: {
          'apps/strata': _pubspec(
            'strata',
            deps:
                '  vendored:\n    path: ../vendored\n'
                '  forked:\n    git: https://example.com/forked.git\n'
                '  flutter_web_plugins:\n    sdk: flutter\n',
            extra: 'dependency_overrides:\n  go_router: 18.0.0\n',
          ),
        },
      );
      expect(runGuard(root), [
        const Violation(
          'strata',
          'dependencies "flutter_web_plugins": unknown SDK package',
        ),
        const Violation(
          'strata',
          'dependencies "forked": path/git dependencies are not allowed; '
              'use a workspace package or a hosted version',
        ),
        const Violation(
          'strata',
          'dependencies "vendored": path/git dependencies are not allowed; '
              'use a workspace package or a hosted version',
        ),
        const Violation('strata', 'dependency_overrides are not allowed'),
      ]);
    });

    test('the workspace root may only hold tooling', () {
      final root = _workspace(rootDevDeps: '  melos: ^8.9.0\n  dio: ^5.0.0\n');
      expect(runGuard(root), [
        const Violation(
          'ws',
          'dev_dependencies "dio" is not allowed: '
              'networking belongs to the Rust core (L15)',
        ),
      ]);
    });

    test('globbed workspace entries are expanded', () {
      final root = _workspace(
        members: {
          'packages/features/tasks': _pubspec(
            'strata_tasks',
            deps: '  dio: ^5.0.0\n',
          ),
        },
      );
      File('${root.path}/pubspec.yaml')
          .writeAsStringSync('name: ws\nworkspace:\n  - packages/features/*\n');
      expect(loadWorkspace(root).map((p) => p.relativeDir), [
        '',
        'packages/features/tasks',
      ]);
      expect(runGuard(root), [
        const Violation(
          'strata_tasks',
          'dependencies "dio" is not allowed: '
              'networking belongs to the Rust core (L15)',
        ),
      ]);
    });
  });

  group('Dart file locations', () {
    const message =
        'Dart files must live in lib/, test/ or tool/ '
        '(L15: no logic outside the UI layers)';

    test('allows lib/, test/, tool/ and generated code', () {
      final root = _workspace(
        members: {'packages/strata_ui': _pubspec('strata_ui')},
        files: {
          'packages/strata_ui/lib/strata_ui.dart': '',
          'packages/strata_ui/lib/src/a.g.dart': '',
          'packages/strata_ui/test/a_test.dart': '',
          'packages/strata_ui/tool/gen.dart': '',
          'tool/logic_guard.dart': '',
          'test/logic_guard_test.dart': '',
          'packages/strata_ui/build/cache/x.dart': '',
          'packages/strata_ui/.dart_tool/x.dart': '',
        },
      );
      expect(runGuard(root), isEmpty);
    });

    test('refuses Dart files anywhere else', () {
      final root = _workspace(
        members: {'apps/strata': _pubspec('strata')},
        files: {
          'apps/strata/bin/server.dart': '',
          'apps/strata/main.dart': '',
          'apps/strata/android/app/src/main/logic.dart': '',
          'scripts/sync.dart': '',
          'helper.dart': '',
        },
      );
      expect(runGuard(root), [
        const Violation('apps/strata/android/app/src/main/logic.dart', message),
        const Violation('apps/strata/bin/server.dart', message),
        const Violation('apps/strata/main.dart', message),
        const Violation('helper.dart', message),
        const Violation('scripts/sync.dart', message),
      ]);
    });

    test('generated detection', () {
      expect(isGenerated('lib/src/router/routes.g.dart'), isTrue);
      expect(
        isGenerated('lib/src/generated/strata_localizations.dart'),
        isTrue,
      );
      expect(isGenerated('lib/src/model.freezed.dart'), isTrue);
      expect(isGenerated('lib/src/logic.dart'), isFalse);
    });
  });

  test('the real Strata workspace passes the guard', () {
    final root = Directory.current;
    expect(File('${root.path}/pubspec.yaml').existsSync(), isTrue);
    final packages = loadWorkspace(root);
    expect(packages.map((p) => p.name), contains('strata_ui'));
    expect(packages.map((p) => p.name), contains('strata_bridge'));
    expect(packages.map((p) => p.name), contains('strata_state'));
    expect(packages, hasLength(19));
    expect(runGuard(root), isEmpty);
  });
}
