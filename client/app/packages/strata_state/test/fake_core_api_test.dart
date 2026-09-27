import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// One Future-returning CoreApi method: how to call it, the call it must
/// record and the answer it must return.
typedef _Case = ({
  Future<Object?> Function(FakeCoreApi api) invoke,
  CoreCall call,
  Object? answer,
});

void main() {
  final draft = StrataFixtures.taskDraft;
  final patch = StrataFixtures.taskPatch;
  const aliases = ['أحمد سمير', 'Ahmed Sameer'];

  final cases = <_Case>[
    (
      invoke: (api) => api.initCore(config: StrataFixtures.coreConfig),
      call: const CoreCall('initCore', {'config': StrataFixtures.coreConfig}),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.signUp(request: StrataFixtures.signUpRequest),
      call: const CoreCall('signUp', {'request': StrataFixtures.signUpRequest}),
      answer: StrataFixtures.signUpOutcome,
    ),
    (
      invoke: (api) => api.signIn(request: StrataFixtures.signInRequest),
      call: const CoreCall('signIn', {'request': StrataFixtures.signInRequest}),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.signOut(force: true),
      call: const CoreCall('signOut', {'force': true}),
      answer: StrataFixtures.signOutOutcome,
    ),
    (
      invoke: (api) => api.switchAccount(userId: 'u-mona'),
      call: const CoreCall('switchAccount', {'userId': 'u-mona'}),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.acknowledgeAccountDisabled(),
      call: const CoreCall('acknowledgeAccountDisabled'),
      answer: StrataFixtures.sessionSignedOut,
    ),
    (
      invoke: (api) => api.refreshAccount(),
      call: const CoreCall('refreshAccount'),
      answer: null,
    ),
    (
      invoke: (api) => api.appLifecycle(state: AppLifecycle.resumed),
      call: const CoreCall('appLifecycle', {'state': AppLifecycle.resumed}),
      answer: null,
    ),
    (
      invoke: (api) => api.syncNow(),
      call: const CoreCall('syncNow'),
      answer: null,
    ),
    (
      invoke: (api) => api.capture(
        text: 'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري',
      ),
      call: const CoreCall('capture', {
        'text': 'كلمت أحمد النهارده، عايزين invoicing أسبوعي بدل شهري',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.createNote(
        path: 'notes/sales/Pricing experiment.md',
        content: '# Pricing experiment\n',
        force: false,
      ),
      call: const CoreCall('createNote', {
        'path': 'notes/sales/Pricing experiment.md',
        'content': '# Pricing experiment\n',
        'force': false,
      }),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) =>
          api.updateNote(id: 'n-pricing-experiments', content: '# Pricing\n'),
      call: const CoreCall('updateNote', {
        'id': 'n-pricing-experiments',
        'content': '# Pricing\n',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.moveNote(
        id: 'n-pricing-experiments',
        newPath: 'notes/archive/Pricing experiments.md',
      ),
      call: const CoreCall('moveNote', {
        'id': 'n-pricing-experiments',
        'newPath': 'notes/archive/Pricing experiments.md',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.deleteNote(id: 'n-churn-notes'),
      call: const CoreCall('deleteNote', {'id': 'n-churn-notes'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.createEntity(
        kind: 'person',
        name: 'Ahmed Samir',
        aliases: aliases,
        force: false,
      ),
      call: const CoreCall('createEntity', {
        'kind': 'person',
        'name': 'Ahmed Samir',
        'aliases': aliases,
        'force': false,
      }),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) => api.addRelation(
        srcId: 'p-ahmed-samir',
        dstId: 'c-acme-logistics',
        relType: 'works-at',
      ),
      call: const CoreCall('addRelation', {
        'srcId': 'p-ahmed-samir',
        'dstId': 'c-acme-logistics',
        'relType': 'works-at',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.removeRelation(
        srcId: 'p-ahmed-samir',
        dstId: 'c-acme-logistics',
        relType: 'works-at',
      ),
      call: const CoreCall('removeRelation', {
        'srcId': 'p-ahmed-samir',
        'dstId': 'c-acme-logistics',
        'relType': 'works-at',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.retypeRelation(
        srcId: 'p-ahmed-samir',
        dstId: 'c-acme-logistics',
        relType: 'works-at',
        newType: 'advises',
      ),
      call: const CoreCall('retypeRelation', {
        'srcId': 'p-ahmed-samir',
        'dstId': 'c-acme-logistics',
        'relType': 'works-at',
        'newType': 'advises',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.acceptSuggestion(id: 's-filing-acme'),
      call: const CoreCall('acceptSuggestion', {'id': 's-filing-acme'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.rejectSuggestion(id: 's-who-is-baba'),
      call: const CoreCall('rejectSuggestion', {'id': 's-who-is-baba'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.requestRelink(noteId: 'n-call-2026-09-12-acme'),
      call: const CoreCall('requestRelink', {
        'noteId': 'n-call-2026-09-12-acme',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.createTask(draft: draft, force: true),
      call: CoreCall('createTask', {'draft': draft, 'force': true}),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) => api.updateTask(taskId: 't-ahmed-proposal', patch: patch),
      call: CoreCall('updateTask', {
        'taskId': 't-ahmed-proposal',
        'patch': patch,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.completeTask(taskId: 't-watanya-eta'),
      call: const CoreCall('completeTask', {'taskId': 't-watanya-eta'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.cancelTask(taskId: 't-nile-freight'),
      call: const CoreCall('cancelTask', {'taskId': 't-nile-freight'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.reopenTask(taskId: 't-watanya-eta-2026-09'),
      call: const CoreCall('reopenTask', {'taskId': 't-watanya-eta-2026-09'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.deleteTask(taskId: 't-petrol-arrows'),
      call: const CoreCall('deleteTask', {'taskId': 't-petrol-arrows'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.resolveConflict(
        opId: '01J8ZQ2A7C4D6F8G0H2J4K6M8N',
        resolution: StrataFixtures.conflictResolution,
      ),
      call: const CoreCall('resolveConflict', {
        'opId': '01J8ZQ2A7C4D6F8G0H2J4K6M8N',
        'resolution': StrataFixtures.conflictResolution,
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.resolveDuplicate(
        opId: '01J8ZQ5N7P9R1T3V5X7Z9B1D3F',
        choice: DuplicateChoice.createAnyway,
      ),
      call: const CoreCall('resolveDuplicate', {
        'opId': '01J8ZQ5N7P9R1T3V5X7Z9B1D3F',
        'choice': DuplicateChoice.createAnyway,
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.dismissRejection(opId: '01J8ZQ1B2C3D4E5F6G7H8J9K0M'),
      call: const CoreCall('dismissRejection', {
        'opId': '01J8ZQ1B2C3D4E5F6G7H8J9K0M',
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.setRemindersEnabled(enabled: false),
      call: const CoreCall('setRemindersEnabled', {'enabled': false}),
      answer: null,
    ),
    (
      invoke: (api) => api.reportNotificationResult(
        id: 4211,
        result: NotificationResult.permissionDenied,
      ),
      call: const CoreCall('reportNotificationResult', {
        'id': 4211,
        'result': NotificationResult.permissionDenied,
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.notificationAction(
        id: 4211,
        action: StrataFixtures.notificationAction,
      ),
      call: const CoreCall('notificationAction', {
        'id': 4211,
        'action': StrataFixtures.notificationAction,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.globalGraph(),
      call: const CoreCall('globalGraph'),
      answer: StrataFixtures.globalGraphView,
    ),
    (
      invoke: (api) => api.search(query: 'pricing', mode: SearchMode.keyword),
      call: const CoreCall('search', {
        'query': 'pricing',
        'mode': SearchMode.keyword,
      }),
      answer: StrataFixtures.searchView,
    ),
    (
      invoke: (api) => api.askView(),
      call: const CoreCall('askView'),
      answer: StrataFixtures.askView,
    ),
    (
      invoke: (api) => api.editorHints(content: '# Call'),
      call: const CoreCall('editorHints', {'content': '# Call'}),
      answer: StrataFixtures.editorHints,
    ),
    (
      invoke: (api) => api.loadAdminUsers(),
      call: const CoreCall('loadAdminUsers'),
      answer: StrataFixtures.adminUsersView,
    ),
  ];

  test('covers every Future-returning CoreApi method once', () {
    // 53 facade functions - 15 streams.
    expect(cases, hasLength(38));
    expect(cases.map((c) => c.call.method).toSet(), hasLength(38));
  });

  for (final c in cases) {
    test('${c.call.method} records its exact arguments and answers', () async {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      expect(await c.invoke(fake), same(c.answer));
      expect(fake.calls, [c.call]);
    });
  }

  group('FakeAnswer', () {
    test('returns a new value, then throws, then returns again', () async {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      fake.createTaskAnswer.returns(StrataFixtures.createOutcomeDuplicate);
      expect(
        await fake.createTask(draft: StrataFixtures.taskDraft, force: false),
        same(StrataFixtures.createOutcomeDuplicate),
      );
      fake.signInAnswer.throws(StrataFixtures.coreFailure);
      await expectLater(
        fake.signIn(request: StrataFixtures.signInRequest),
        throwsA(same(StrataFixtures.coreFailure)),
      );
      fake.signInAnswer.returns(StrataFixtures.sessionSignedOut);
      expect(
        await fake.signIn(request: StrataFixtures.signInRequest),
        same(StrataFixtures.sessionSignedOut),
      );
    });
  });

  group('FakeStream', () {
    test(
      'replays the latest value to a new subscriber, then live ones',
      () async {
        final fake = FakeCoreApi();
        addTearDown(fake.dispose);
        fake.home
          ..add(StrataFixtures.homeView)
          ..add(StrataFixtures.homeView);
        final seen = <HomeView>[];
        final sub = fake.watchHome().listen(seen.add);
        addTearDown(sub.cancel);
        await Future<void>.delayed(Duration.zero);
        expect(seen, [same(StrataFixtures.homeView)]);
        final empty = HomeView(
          recentNotes: const [],
          inboxCount: 0,
          tasks: StrataFixtures.taskSections,
          sync_: StrataFixtures.syncPill,
        );
        fake.home.add(empty);
        await Future<void>.delayed(Duration.zero);
        expect(seen, [same(StrataFixtures.homeView), same(empty)]);
      },
    );

    test('forwards errors and closes', () async {
      final fake = FakeCoreApi();
      final events = <Object>[];
      final done = fake
          .watchSettings()
          .listen(events.add, onError: events.add)
          .asFuture<void>();
      fake.settings
        ..add(StrataFixtures.settingsView)
        ..addError(StrataFixtures.coreFailure);
      fake.dispose();
      await done.catchError((_) {});
      expect(events, [
        same(StrataFixtures.settingsView),
        same(StrataFixtures.coreFailure),
      ]);
    });

    test('families keep one stream per argument', () {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      expect(fake.note['a'], same(fake.note['a']));
      expect(fake.note['a'], isNot(same(fake.note['b'])));
      expect(
        fake.directory[(DirectoryTab.places, 'safe')],
        same(fake.directory[(DirectoryTab.places, 'safe')]),
      );
      expect(fake.note.keys, ['a', 'b']);
    });
  });

  test('CoreCall compares method and arguments by value', () {
    expect(
      const CoreCall('watchNote', {'id': 'a'}),
      CoreCall('watchNote', {'id': 'a'}),
    );
    expect(
      const CoreCall('watchNote', {'id': 'a'}),
      isNot(const CoreCall('watchNote', {'id': 'b'})),
    );
    expect(
      const CoreCall('watchNote', {'id': 'a'}).toString(),
      'watchNote({id: a})',
    );
  });
}
