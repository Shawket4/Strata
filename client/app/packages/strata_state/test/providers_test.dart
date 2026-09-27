import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show ProviderListenable;
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

/// A container whose core is [fake].
ProviderContainer _container(FakeCoreApi fake) {
  final container = ProviderContainer(
    overrides: [coreApiProvider.overrideWithValue(fake)],
  );
  addTearDown(container.dispose);
  addTearDown(fake.dispose);
  return container;
}

/// Listens to [provider] and records every value it exposes.
List<AsyncValue<T>> _record<T>(
  ProviderContainer container,
  ProviderListenable<AsyncValue<T>> provider,
) {
  final seen = <AsyncValue<T>>[];
  container.listen(provider, (_, next) => seen.add(next), fireImmediately: true);
  return seen;
}

/// Lets queued stream events reach the providers.
Future<void> _flush() => Future<void>.delayed(Duration.zero);

/// The values (not loading/error states) in [seen].
List<T> _values<T>(List<AsyncValue<T>> seen) => [
  for (final v in seen)
    if (v is AsyncData<T>) v.value,
];

/// Checks one stream provider: it is loading until the fake emits, then
/// exposes exactly the fake's values in order (same instances), and it
/// subscribed with exactly [call].
Future<void> _expectStream<T>({
  required FakeCoreApi fake,
  required ProviderListenable<AsyncValue<T>> provider,
  required FakeStream<T> stream,
  required List<T> values,
  required CoreCall call,
}) async {
  final container = _container(fake);
  final seen = _record(container, provider);
  await _flush();
  expect(seen, [isA<AsyncLoading<T>>()]);
  expect(fake.calls, [call]);
  for (final value in values) {
    stream.add(value);
    await _flush();
  }
  final emitted = _values(seen);
  expect(emitted, hasLength(values.length));
  for (var i = 0; i < values.length; i++) {
    expect(emitted[i], same(values[i]));
  }
  expect(fake.calls, [call], reason: 'one subscription, no re-reads');
}

void main() {
  group('coreApiProvider', () {
    test('has no default: reading it unoverridden fails', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      expect(
        () => container.read(coreApiProvider),
        throwsA(
          isA<Object>().having(
            (e) => e.toString(),
            'message',
            contains('coreApiProvider must be overridden'),
          ),
        ),
      );
    });

    test('an override is what every provider reads', () async {
      final fake = FakeCoreApi();
      final container = _container(fake);
      expect(container.read(coreApiProvider), same(fake));
      fake.home.add(StrataFixtures.homeView);
      final seen = _record(container, homeProvider);
      await _flush();
      expect(_values(seen), [same(StrataFixtures.homeView)]);
    });

    test('each container reads its own override', () async {
      final signedOut = FakeCoreApi()..session.add(StrataFixtures.sessionSignedOut);
      final active = FakeCoreApi()..session.add(StrataFixtures.sessionActive);
      final first = _record(_container(signedOut), sessionProvider);
      final second = _record(_container(active), sessionProvider);
      await _flush();
      expect(_values(first), [same(StrataFixtures.sessionSignedOut)]);
      expect(_values(second), [same(StrataFixtures.sessionActive)]);
    });

    test('BridgeCoreApi is a CoreApi', () {
      expect(const BridgeCoreApi(), isA<CoreApi>());
    });
  });

  group('app-wide streams emit exactly what the core emits', () {
    test('sessionProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: sessionProvider,
        stream: fake.session,
        values: [
          StrataFixtures.sessionSignedOut,
          StrataFixtures.sessionActive,
          StrataFixtures.sessionDeletionPending,
        ],
        call: const CoreCall('watchSession'),
      );
    });

    test('syncStatusProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: syncStatusProvider,
        stream: fake.syncStatus,
        values: [
          StrataFixtures.syncStatusView,
          SyncStatusView(
            pill: StrataFixtures.syncPillOffline,
            bootstrapComplete: true,
            outbox: const [],
            conflicts: const [],
            rejections: const [],
          ),
        ],
        call: const CoreCall('watchSyncStatus'),
      );
    });

    test('notificationOpsProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: notificationOpsProvider,
        stream: fake.notificationOps,
        values: [
          StrataFixtures.notificationOp,
          const NotificationOp(
            kind: NotificationOpKind.cancel,
            id: 4211,
            title: '',
            body: '',
            taskId: 't-petrol-arrows',
          ),
        ],
        call: const CoreCall('watchNotificationOps'),
      );
    });
  });

  group('screen streams emit exactly what the core emits', () {
    test('homeProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: homeProvider,
        stream: fake.home,
        values: [
          StrataFixtures.homeView,
          HomeView(
            recentNotes: const [],
            inboxCount: 0,
            tasks: StrataFixtures.taskSections,
            sync_: StrataFixtures.syncPillOffline,
          ),
        ],
        call: const CoreCall('watchHome'),
      );
    });

    test('inboxProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: inboxProvider,
        stream: fake.inbox,
        values: [StrataFixtures.inboxView],
        call: const CoreCall('watchInbox'),
      );
    });

    test('noteProvider(id)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: noteProvider('n-call-2026-09-12-acme'),
        stream: fake.note['n-call-2026-09-12-acme'],
        values: [
          StrataFixtures.noteScreen,
          const NoteScreen(id: 'n-call-2026-09-12-acme'),
        ],
        call: const CoreCall('watchNote', {'id': 'n-call-2026-09-12-acme'}),
      );
    });

    test('notesListProvider(folder)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: notesListProvider('notes'),
        stream: fake.notesList['notes'],
        values: [StrataFixtures.notesListView],
        call: const CoreCall('watchNotesList', {'folder': 'notes'}),
      );
    });

    test('directoryProvider(tab, query)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: directoryProvider(DirectoryTab.people, 'أحمد'),
        stream: fake.directory[(DirectoryTab.people, 'أحمد')],
        values: [StrataFixtures.directoryView],
        call: const CoreCall('watchDirectory', {
          'tab': DirectoryTab.people,
          'query': 'أحمد',
        }),
      );
    });

    test('entityProvider(id)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: entityProvider('d-watanya-contract'),
        stream: fake.entity['d-watanya-contract'],
        values: [StrataFixtures.entityScreenDocument],
        call: const CoreCall('watchEntity', {'id': 'd-watanya-contract'}),
      );
    });

    test('tasksProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: tasksProvider,
        stream: fake.tasks,
        values: [StrataFixtures.tasksView],
        call: const CoreCall('watchTasks'),
      );
    });

    test('taskProvider(id)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: taskProvider('t-watanya-eta'),
        stream: fake.task['t-watanya-eta'],
        values: [StrataFixtures.taskScreen],
        call: const CoreCall('watchTask', {'id': 't-watanya-eta'}),
      );
    });

    test('conflictProvider(opId)', () async {
      final fake = FakeCoreApi();
      const opId = '01J8ZQ2A7C4D6F8G0H2J4K6M8N';
      await _expectStream(
        fake: fake,
        provider: conflictProvider(opId),
        stream: fake.conflict[opId],
        values: [
          StrataFixtures.conflictScreen,
          const ConflictScreen(opId: opId),
        ],
        call: const CoreCall('watchConflict', {'opId': opId}),
      );
    });

    test('duplicatePromptsProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: duplicatePromptsProvider,
        stream: fake.duplicatePrompts,
        values: [StrataFixtures.duplicatePromptsView],
        call: const CoreCall('watchDuplicatePrompts'),
      );
    });

    test('settingsProvider', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: settingsProvider,
        stream: fake.settings,
        values: [StrataFixtures.settingsView],
        call: const CoreCall('watchSettings'),
      );
    });

    test('localGraphProvider(id, depth)', () async {
      final fake = FakeCoreApi();
      await _expectStream(
        fake: fake,
        provider: localGraphProvider('p-ahmed-samir', 2),
        stream: fake.localGraph[('p-ahmed-samir', 2)],
        values: [StrataFixtures.localGraphView],
        call: const CoreCall('watchLocalGraph', {
          'id': 'p-ahmed-samir',
          'depth': 2,
        }),
      );
    });

    test('family members are independent streams', () async {
      final fake = FakeCoreApi();
      final container = _container(fake);
      final ahmed = _record(container, entityProvider('p-ahmed-samir'));
      final safe = _record(container, entityProvider('pl-nasr-city-office'));
      fake.entity['p-ahmed-samir'].add(StrataFixtures.entityScreen);
      await _flush();
      expect(_values(ahmed), [same(StrataFixtures.entityScreen)]);
      expect(_values(safe), isEmpty);
      expect(fake.calls, const [
        CoreCall('watchEntity', {'id': 'p-ahmed-samir'}),
        CoreCall('watchEntity', {'id': 'pl-nasr-city-office'}),
      ]);
    });

    testWidgets('a core error is exposed as is and never retried', (
      tester,
    ) async {
      final fake = FakeCoreApi();
      final container = _container(fake);
      final seen = _record(container, homeProvider);
      fake.home.addError(StrataFixtures.coreFailure);
      await tester.pump();
      expect(seen.last, isA<AsyncError<HomeView>>());
      expect(seen.last.error, same(StrataFixtures.coreFailure));
      // Riverpod's default policy would re-subscribe within seconds.
      await tester.pump(const Duration(minutes: 5));
      expect(fake.calls, const [CoreCall('watchHome')]);
      expect(seen.last.error, same(StrataFixtures.coreFailure));
    });
  });

  group('one-shots return exactly what the core returns', () {
    Future<void> expectOneShot<T>(
      FakeCoreApi fake,
      ProviderListenable<Future<T>> provider,
      T value,
      CoreCall call,
    ) async {
      final container = _container(fake);
      expect(await container.read(provider), same(value));
      expect(fake.calls, [call]);
    }

    test('globalGraphProvider', () async {
      final fake = FakeCoreApi();
      await expectOneShot(
        fake,
        globalGraphProvider.future,
        StrataFixtures.globalGraphView,
        const CoreCall('globalGraph'),
      );
    });

    test('searchProvider(query, mode)', () async {
      final fake = FakeCoreApi();
      const view = SearchView(
        query: 'تسعير',
        mode: SearchMode.hybrid,
        results: [],
        availability: Availability.offline,
      );
      fake.searchAnswer.returns(view);
      await expectOneShot(
        fake,
        searchProvider('تسعير', SearchMode.hybrid).future,
        view,
        const CoreCall('search', {'query': 'تسعير', 'mode': SearchMode.hybrid}),
      );
    });

    test('askViewProvider', () async {
      final fake = FakeCoreApi();
      await expectOneShot(
        fake,
        askViewProvider.future,
        StrataFixtures.askView,
        const CoreCall('askView'),
      );
    });

    test('editorHintsProvider(content)', () async {
      final fake = FakeCoreApi();
      await expectOneShot(
        fake,
        editorHintsProvider('# Call\n[[Ahmed Samir]]').future,
        StrataFixtures.editorHints,
        const CoreCall('editorHints', {'content': '# Call\n[[Ahmed Samir]]'}),
      );
    });

    test('adminUsersProvider', () async {
      final fake = FakeCoreApi();
      await expectOneShot(
        fake,
        adminUsersProvider.future,
        StrataFixtures.adminUsersView,
        const CoreCall('loadAdminUsers'),
      );
    });

    test('a failing one-shot surfaces the core error unchanged', () async {
      final fake = FakeCoreApi();
      const failure = CoreFailure(
        code: 'not_allowed',
        messageKey: 'error.not_allowed',
        status: 403,
      );
      fake.loadAdminUsersAnswer.throws(failure);
      final container = _container(fake);
      await expectLater(
        container.read(adminUsersProvider.future),
        throwsA(same(failure)),
      );
      expect(fake.calls, const [CoreCall('loadAdminUsers')]);
    });
  });

  test('stream providers stay subscribed until disposed', () async {
    final fake = FakeCoreApi();
    final container = _container(fake);
    final sub = container.listen(homeProvider, (_, _) {});
    await _flush();
    expect(fake.home.hasListener, isTrue);
    sub.close();
    await _flush();
    expect(fake.home.hasListener, isFalse, reason: 'autoDispose');
    unawaited(container.read(sessionProvider.future));
    await _flush();
    expect(fake.session.hasListener, isTrue, reason: 'keepAlive');
  });
}
