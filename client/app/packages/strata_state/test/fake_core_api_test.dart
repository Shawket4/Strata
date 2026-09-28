import 'dart:async';

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
      invoke: (api) => api.refresh(),
      call: const CoreCall('refresh'),
      answer: null,
    ),
    (
      invoke: (api) => api.checkApproval(),
      call: const CoreCall('checkApproval'),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.dismissPending(),
      call: const CoreCall('dismissPending'),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.passwordStrength(password: 'password'),
      call: const CoreCall('passwordStrength', {'password': 'password'}),
      answer: StrataFixtures.passwordStrength,
    ),
    (
      invoke: (api) => api.changePassword(current: 'current', new_: 'new_'),
      call: const CoreCall('changePassword', {
        'current': 'current',
        'new_': 'new_',
      }),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.setUiLanguage(code: 'code'),
      call: const CoreCall('setUiLanguage', {'code': 'code'}),
      answer: null,
    ),
    (
      invoke: (api) => api.setTimezone(iana: 'iana'),
      call: const CoreCall('setTimezone', {'iana': 'iana'}),
      answer: null,
    ),
    (
      invoke: (api) => api.setDisplayName(name: 'name'),
      call: const CoreCall('setDisplayName', {'name': 'name'}),
      answer: null,
    ),
    (
      invoke: (api) => api.downloadExport(path: 'path'),
      call: const CoreCall('downloadExport', {'path': 'path'}),
      answer: StrataFixtures.exportSummary,
    ),
    (
      invoke: (api) => api.deleteAccountNow(force: true),
      call: const CoreCall('deleteAccountNow', {'force': true}),
      answer: StrataFixtures.sessionActive,
    ),
    (
      invoke: (api) => api.exportUnsynced(path: 'path'),
      call: const CoreCall('exportUnsynced', {'path': 'path'}),
      answer: 0,
    ),
    (
      invoke: (api) => api.setSyncPaused(paused: true),
      call: const CoreCall('setSyncPaused', {'paused': true}),
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
      invoke: (api) => api.updateNote(
        id: 'id',
        content: 'content',
        baseVersion: 'baseVersion',
      ),
      call: const CoreCall('updateNote', {
        'id': 'id',
        'content': 'content',
        'baseVersion': 'baseVersion',
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
      invoke: (api) =>
          api.addReminder(taskId: 'taskId', at: StrataFixtures.now),
      call: CoreCall('addReminder', {
        'taskId': 'taskId',
        'at': StrataFixtures.now,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) =>
          api.removeReminder(taskId: 'taskId', at: StrataFixtures.now),
      call: CoreCall('removeReminder', {
        'taskId': 'taskId',
        'at': StrataFixtures.now,
      }),
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
      invoke: (api) => api.insertMention(
        noteId: 'noteId',
        content: 'content',
        start: 3,
        end: 3,
        entityId: 'entityId',
      ),
      call: const CoreCall('insertMention', {
        'noteId': 'noteId',
        'content': 'content',
        'start': 3,
        'end': 3,
        'entityId': 'entityId',
      }),
      answer: StrataFixtures.mentionEdit,
    ),
    (
      invoke: (api) => api.pinNote(id: 'id', pinned: true),
      call: const CoreCall('pinNote', {'id': 'id', 'pinned': true}),
      answer: null,
    ),
    (
      invoke: (api) => api.acceptCapture(noteId: 'noteId'),
      call: const CoreCall('acceptCapture', {'noteId': 'noteId'}),
      answer: const <String>[],
    ),
    (
      invoke: (api) => api.rejectCapture(noteId: 'noteId'),
      call: const CoreCall('rejectCapture', {'noteId': 'noteId'}),
      answer: const <String>[],
    ),
    (
      invoke: (api) => api.acceptCaptures(noteIds: const ['a']),
      call: const CoreCall('acceptCaptures', {
        'noteIds': ['a'],
      }),
      answer: const <String>[],
    ),
    (
      invoke: (api) => api.acceptAllReady(),
      call: const CoreCall('acceptAllReady'),
      answer: const <String>[],
    ),
    (
      invoke: (api) => api.acceptSuggestionWith(
        id: 'id',
        edits: StrataFixtures.suggestionEdits,
      ),
      call: const CoreCall('acceptSuggestionWith', {
        'id': 'id',
        'edits': StrataFixtures.suggestionEdits,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.resolveLinkOrCreate(
        id: 'id',
        choice: StrataFixtures.linkOrCreateChoice,
      ),
      call: const CoreCall('resolveLinkOrCreate', {
        'id': 'id',
        'choice': StrataFixtures.linkOrCreateChoice,
      }),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) =>
          api.acceptSuggestionChoice(id: 'id', documentId: 'documentId'),
      call: const CoreCall('acceptSuggestionChoice', {
        'id': 'id',
        'documentId': 'documentId',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.undoSuggestion(id: 'id'),
      call: const CoreCall('undoSuggestion', {'id': 'id'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.acknowledgeSuggestion(id: 'id'),
      call: const CoreCall('acknowledgeSuggestion', {'id': 'id'}),
      answer: null,
    ),
    (
      invoke: (api) => api.resolveCaptureDuplicate(
        id: 'id',
        choice: DuplicateChoice.createAnyway,
      ),
      call: const CoreCall('resolveCaptureDuplicate', {
        'id': 'id',
        'choice': DuplicateChoice.createAnyway,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.replyToSuggestion(id: 'id', text: 'text'),
      call: const CoreCall('replyToSuggestion', {'id': 'id', 'text': 'text'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) =>
          api.createDocument(draft: StrataFixtures.documentDraft, force: true),
      call: const CoreCall('createDocument', {
        'draft': StrataFixtures.documentDraft,
        'force': true,
      }),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) =>
          api.createPlace(draft: StrataFixtures.placeDraft, force: true),
      call: const CoreCall('createPlace', {
        'draft': StrataFixtures.placeDraft,
        'force': true,
      }),
      answer: StrataFixtures.createOutcomeCreated,
    ),
    (
      invoke: (api) =>
          api.mergeEntities(sourceId: 'sourceId', intoId: 'intoId'),
      call: const CoreCall('mergeEntities', {
        'sourceId': 'sourceId',
        'intoId': 'intoId',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.repointRelation(
        srcId: 'srcId',
        dstId: 'dstId',
        relType: 'relType',
        newDstId: 'newDstId',
      ),
      call: const CoreCall('repointRelation', {
        'srcId': 'srcId',
        'dstId': 'dstId',
        'relType': 'relType',
        'newDstId': 'newDstId',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.rejectRelation(
        srcId: 'srcId',
        dstId: 'dstId',
        relType: 'relType',
      ),
      call: const CoreCall('rejectRelation', {
        'srcId': 'srcId',
        'dstId': 'dstId',
        'relType': 'relType',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.updateUserNotes(id: 'id', text: 'text'),
      call: const CoreCall('updateUserNotes', {'id': 'id', 'text': 'text'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.setProperty(id: 'id', key: 'key', value: 'value'),
      call: const CoreCall('setProperty', {
        'id': 'id',
        'key': 'key',
        'value': 'value',
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.setPropertyValues(
        id: 'id',
        key: 'phone',
        values: const ['1', '2'],
      ),
      call: const CoreCall('setPropertyValues', {
        'id': 'id',
        'key': 'phone',
        'values': ['1', '2'],
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.removeProperty(id: 'id', key: 'key'),
      call: const CoreCall('removeProperty', {'id': 'id', 'key': 'key'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.addAlias(id: 'id', alias: 'alias'),
      call: const CoreCall('addAlias', {'id': 'id', 'alias': 'alias'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.removeAlias(id: 'id', alias: 'alias'),
      call: const CoreCall('removeAlias', {'id': 'id', 'alias': 'alias'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.recordCustody(
        documentId: 'documentId',
        draft: StrataFixtures.custodyDraft,
      ),
      call: CoreCall('recordCustody', {
        'documentId': 'documentId',
        'draft': StrataFixtures.custodyDraft,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.setDefaultReminderTime(time: 'time'),
      call: const CoreCall('setDefaultReminderTime', {'time': 'time'}),
      answer: null,
    ),
    (
      invoke: (api) =>
          api.setQuietHours(enabled: true, from: 'from', until: 'until'),
      call: const CoreCall('setQuietHours', {
        'enabled': true,
        'from': 'from',
        'until': 'until',
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.setSnoozeMinutes(minutes: 3),
      call: const CoreCall('setSnoozeMinutes', {'minutes': 3}),
      answer: null,
    ),
    (
      invoke: (api) => api.refreshSettings(),
      call: const CoreCall('refreshSettings'),
      answer: null,
    ),
    (
      invoke: (api) => api.retryFailedJobs(),
      call: const CoreCall('retryFailedJobs'),
      answer: 0,
    ),
    (
      invoke: (api) => api.renameDevice(id: 'id', name: 'name'),
      call: const CoreCall('renameDevice', {'id': 'id', 'name': 'name'}),
      answer: null,
    ),
    (
      invoke: (api) => api.revokeDevice(id: 'id'),
      call: const CoreCall('revokeDevice', {'id': 'id'}),
      answer: null,
    ),
    (
      invoke: (api) => api.setDeviceReminders(id: 'id', enabled: true),
      call: const CoreCall('setDeviceReminders', {'id': 'id', 'enabled': true}),
      answer: null,
    ),
    (
      invoke: (api) => api.refreshHistory(noteId: 'noteId'),
      call: const CoreCall('refreshHistory', {'noteId': 'noteId'}),
      answer: null,
    ),
    (
      invoke: (api) => api.revertNote(noteId: 'noteId', commit: 'commit'),
      call: const CoreCall('revertNote', {
        'noteId': 'noteId',
        'commit': 'commit',
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.exportVault(path: 'path'),
      call: const CoreCall('exportVault', {'path': 'path'}),
      answer: StrataFixtures.exportSummary,
    ),
    (
      invoke: (api) => api.importVault(path: 'path'),
      call: const CoreCall('importVault', {'path': 'path'}),
      answer: StrataFixtures.importSummary,
    ),
    (
      invoke: (api) => api.approveUser(id: 'id'),
      call: const CoreCall('approveUser', {'id': 'id'}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.rejectUser(id: 'id'),
      call: const CoreCall('rejectUser', {'id': 'id'}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.setUserRole(id: 'id', role: 'role'),
      call: const CoreCall('setUserRole', {'id': 'id', 'role': 'role'}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.setUserEnabled(id: 'id', enabled: true),
      call: const CoreCall('setUserEnabled', {'id': 'id', 'enabled': true}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.resetPassword(id: 'id'),
      call: const CoreCall('resetPassword', {'id': 'id'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.scheduleDeletion(id: 'id'),
      call: const CoreCall('scheduleDeletion', {'id': 'id'}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.cancelDeletion(id: 'id'),
      call: const CoreCall('cancelDeletion', {'id': 'id'}),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) => api.createUser(request: StrataFixtures.newUserRequest),
      call: const CoreCall('createUser', {
        'request': StrataFixtures.newUserRequest,
      }),
      answer: StrataFixtures.adminUserItem,
    ),
    (
      invoke: (api) =>
          api.ask(question: 'question', scope: StrataFixtures.askScope),
      call: const CoreCall('ask', {
        'question': 'question',
        'scope': StrataFixtures.askScope,
      }),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.openNoteThread(id: 'n'),
      call: const CoreCall('openNoteThread', {'id': 'n'}),
      answer: null,
    ),
    (
      invoke: (api) => api.stopAsk(),
      call: const CoreCall('stopAsk'),
      answer: null,
    ),
    (
      invoke: (api) => api.newConversation(),
      call: const CoreCall('newConversation'),
      answer: null,
    ),
    (
      invoke: (api) => api.saveAnswerAsNote(messageId: 'messageId'),
      call: const CoreCall('saveAnswerAsNote', {'messageId': 'messageId'}),
      answer: StrataFixtures.opId,
    ),
    (
      invoke: (api) => api.refreshAiActivity(),
      call: const CoreCall('refreshAiActivity'),
      answer: null,
    ),
    (
      invoke: (api) => api.rejectAiDecision(decisionId: 'decisionId'),
      call: const CoreCall('rejectAiDecision', {'decisionId': 'decisionId'}),
      answer: null,
    ),
    (
      invoke: (api) => api.repointAiDecision(
        decisionId: 'decisionId',
        targetId: 'targetId',
        hint: 'hint',
      ),
      call: const CoreCall('repointAiDecision', {
        'decisionId': 'decisionId',
        'targetId': 'targetId',
        'hint': 'hint',
      }),
      answer: null,
    ),
    (
      invoke: (api) =>
          api.retypeAiDecision(decisionId: 'decisionId', relType: 'relType'),
      call: const CoreCall('retypeAiDecision', {
        'decisionId': 'decisionId',
        'relType': 'relType',
      }),
      answer: null,
    ),
    (
      invoke: (api) => api.refreshSimilarity(),
      call: const CoreCall('refreshSimilarity'),
      answer: null,
    ),
    (
      invoke: (api) => api.saveLayout(
        centerId: 'centerId',
        name: 'name',
        positions: const [StrataFixtures.nodePosition],
      ),
      call: const CoreCall('saveLayout', {
        'centerId': 'centerId',
        'name': 'name',
        'positions': [StrataFixtures.nodePosition],
      }),
      answer: StrataFixtures.opId,
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
      invoke: (api) =>
          api.globalGraphFiltered(filter: StrataFixtures.graphFilter),
      call: const CoreCall('globalGraphFiltered', {
        'filter': StrataFixtures.graphFilter,
      }),
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
      invoke: (api) => api.searchInFolder(
        query: 'query',
        mode: SearchMode.keyword,
        folder: 'folder',
      ),
      call: const CoreCall('searchInFolder', {
        'query': 'query',
        'mode': SearchMode.keyword,
        'folder': 'folder',
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
      invoke: (api) => api.editorCompletions(
        noteId: 'noteId',
        content: 'content',
        cursor: 3,
      ),
      call: const CoreCall('editorCompletions', {
        'noteId': 'noteId',
        'content': 'content',
        'cursor': 3,
      }),
      answer: StrataFixtures.completions,
    ),
    (
      invoke: (api) => api.tags(prefix: 'prefix'),
      call: const CoreCall('tags', {'prefix': 'prefix'}),
      answer: const <TagItem>[],
    ),
    (
      invoke: (api) => api.noteBlocks(noteId: 'noteId'),
      call: const CoreCall('noteBlocks', {'noteId': 'noteId'}),
      answer: const <BlockItem>[],
    ),
    (
      invoke: (api) => api.relationTypes(),
      call: const CoreCall('relationTypes'),
      answer: const <RelationTypeItem>[],
    ),
    (
      invoke: (api) => api.recurrenceForm(phrase: 'phrase'),
      call: const CoreCall('recurrenceForm', {'phrase': 'phrase'}),
      answer: null,
    ),
    (
      invoke: (api) =>
          api.composeRecurrence(form: StrataFixtures.recurrenceForm),
      call: CoreCall('composeRecurrence', {
        'form': StrataFixtures.recurrenceForm,
      }),
      answer: StrataFixtures.recurrenceCompose,
    ),
    (
      invoke: (api) => api.recurrencePreview(
        phrase: 'phrase',
        from: StrataFixtures.now,
        count: 3,
      ),
      call: CoreCall('recurrencePreview', {
        'phrase': 'phrase',
        'from': StrataFixtures.now,
        'count': 3,
      }),
      answer: const <RecurrencePreviewItem>[],
    ),
    (
      invoke: (api) => api.parseTaskText(text: 'text'),
      call: const CoreCall('parseTaskText', {'text': 'text'}),
      answer: StrataFixtures.taskDraftPreview,
    ),
    (
      invoke: (api) => api.placeOptions(documentId: 'documentId'),
      call: const CoreCall('placeOptions', {'documentId': 'documentId'}),
      answer: const <PlaceOption>[],
    ),
    (
      invoke: (api) => api.mergePreview(sourceId: 'sourceId', intoId: 'intoId'),
      call: const CoreCall('mergePreview', {
        'sourceId': 'sourceId',
        'intoId': 'intoId',
      }),
      answer: StrataFixtures.mergePreview,
    ),
    (
      invoke: (api) => api.resolveCitation(noteId: 'noteId', anchor: 'anchor'),
      call: const CoreCall('resolveCitation', {
        'noteId': 'noteId',
        'anchor': 'anchor',
      }),
      answer: StrataFixtures.citationPreview,
    ),
    (
      invoke: (api) => api.noteRevisionDiff(noteId: 'noteId', commit: 'commit'),
      call: const CoreCall('noteRevisionDiff', {
        'noteId': 'noteId',
        'commit': 'commit',
      }),
      answer: StrataFixtures.noteDiffView,
    ),
    (
      invoke: (api) => api.timezones(query: 'cairo'),
      call: const CoreCall('timezones', {'query': 'cairo'}),
      answer: StrataFixtures.timezones,
    ),
    (
      invoke: (api) => api.repointChoices(decisionId: 'd', query: 'q'),
      call: const CoreCall('repointChoices', {'decisionId': 'd', 'query': 'q'}),
      answer: StrataFixtures.repointChoices,
    ),
    (
      invoke: (api) => api.loadAdminUsers(query: 'query'),
      call: const CoreCall('loadAdminUsers', {'query': 'query'}),
      answer: StrataFixtures.adminUsersView,
    ),
  ];

  test('covers every Future-returning CoreApi method once', () {
    // 143 facade functions - 22 streams.
    expect(cases, hasLength(124));
    expect(cases.map((c) => c.call.method).toSet(), hasLength(124));
  });

  for (final c in cases) {
    test('${c.call.method} records its exact arguments and answers', () async {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      expect(await c.invoke(fake), same(c.answer));
      expect(fake.calls, [c.call]);
    });
  }

  group('FakeFilePicker', () {
    test('records the dialogs in the fake core calls and answers', () async {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      expect(
        await fake.files.saveFile(
          suggestedName: 'strata-vault.zip',
          type: PickedFileType.zip,
        ),
        '/home/shawket/Downloads/strata-vault.zip',
      );
      fake.files.openFileAnswer.returns(null);
      expect(await fake.files.openFile(type: PickedFileType.markdown), isNull);
      expect(fake.calls, [
        const CoreCall('saveFile', {
          'suggestedName': 'strata-vault.zip',
          'type': PickedFileType.zip,
        }),
        const CoreCall('openFile', {'type': PickedFileType.markdown}),
      ]);
    });
  });

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
          todayLabel: '',
          greeting: '',
          displayName: '',
          inboxPreview: [],
          needsYouCount: 0,
          contradictionsCount: 0,
          inboxSummary: '',
          aiActivity: Availability.available,
          aiActivityItems: [],
          aiActivityHeadline: '',
          openItems: Availability.available,
          openItemList: [],
          pinned: [],
        );
        fake.home.add(empty);
        await Future<void>.delayed(Duration.zero);
        expect(seen, [same(StrataFixtures.homeView), same(empty)]);
      },
    );

    test('forwards errors and closes', () async {
      final fake = FakeCoreApi();
      final events = <Object>[];
      final done = Completer<void>();
      fake.watchSettings().listen(
        events.add,
        onError: events.add,
        onDone: done.complete,
      );
      fake.settings
        ..add(StrataFixtures.settingsView)
        ..addError(StrataFixtures.coreFailure);
      fake.dispose();
      await done.future;
      expect(events, [
        same(StrataFixtures.settingsView),
        same(StrataFixtures.coreFailure),
      ]);
    });

    test('keeps order when live values follow a pending replay', () async {
      final fake = FakeCoreApi();
      addTearDown(fake.dispose);
      final first = StrataFixtures.homeView;
      final second = HomeView(
        recentNotes: const [],
        inboxCount: 1,
        tasks: StrataFixtures.taskSections,
        sync_: StrataFixtures.syncPillOffline,
        todayLabel: '',
        greeting: '',
        displayName: '',
        inboxPreview: [],
        needsYouCount: 0,
        contradictionsCount: 0,
        inboxSummary: '',
        aiActivity: Availability.available,
        aiActivityItems: [],
        aiActivityHeadline: '',
        openItems: Availability.available,
        openItemList: [],
        pinned: [],
      );
      fake.home.add(first);
      final seen = <HomeView>[];
      final sub = fake.watchHome().listen(seen.add);
      addTearDown(sub.cancel);
      fake.home.add(second);
      await Future<void>.delayed(Duration.zero);
      expect(seen, [same(first), same(second)]);
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
      CoreCall('watchNote', {'id': 'a'.toLowerCase()}),
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
