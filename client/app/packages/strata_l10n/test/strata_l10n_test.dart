import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_l10n/strata_l10n.dart';

Map<String, Object?> _readArb(String name) =>
    jsonDecode(File('lib/l10n/$name').readAsStringSync())
        as Map<String, Object?>;

Future<StrataLocalizations> _load(Locale locale) =>
    StrataLocalizations.delegate.load(locale);

Future<({TextDirection direction, StrataLocalizations l10n})> _pumpIn(
  WidgetTester tester,
  Locale locale,
) async {
  late TextDirection direction;
  late StrataLocalizations l10n;
  await tester.pumpWidget(
    Localizations(
      locale: locale,
      delegates: StrataLocalizations.localizationsDelegates,
      child: Builder(
        builder: (context) {
          direction = Directionality.of(context);
          l10n = context.l10n;
          return const SizedBox.shrink();
        },
      ),
    ),
  );
  return (direction: direction, l10n: l10n);
}

void main() {
  group('ARB files', () {
    test('Arabic translates every English message and nothing else', () {
      final en = _readArb('strata_en.arb');
      final ar = _readArb('strata_ar.arb');
      Set<String> messages(Map<String, Object?> arb) =>
          arb.keys.where((k) => !k.startsWith('@')).toSet();
      expect(messages(ar), equals(messages(en)));
      expect(en['@@locale'], 'en');
      expect(ar['@@locale'], 'ar');
    });

    test('every English message carries a description', () {
      final en = _readArb('strata_en.arb');
      final undocumented = en.keys
          .where((k) => !k.startsWith('@'))
          .where((k) {
            final meta = en['@$k'];
            return meta is! Map || (meta['description'] as String?) == null;
          })
          .toList();
      expect(undocumented, isEmpty);
    });
  });

  group('StrataLocalizations', () {
    test('supports exactly English and Arabic', () {
      expect(
        StrataLocalizations.supportedLocales,
        unorderedEquals(const [StrataLocales.english, StrataLocales.arabic]),
      );
      expect(
        StrataLocalizations.delegate.isSupported(const Locale('fr')),
        isFalse,
      );
    });

    test('English navigation labels and common actions', () async {
      final l10n = await _load(StrataLocales.english);
      expect(
        [
          l10n.navHome,
          l10n.navInbox,
          l10n.navTasks,
          l10n.navNotes,
          l10n.navMap,
          l10n.navDirectory,
          l10n.navAsk,
          l10n.navSettings,
        ],
        [
          'Home',
          'Inbox',
          'Tasks',
          'Notes',
          'Map',
          'Directory',
          'Ask',
          'Settings',
        ],
      );
      expect(l10n.actionNewCapture, 'New capture');
      expect(l10n.actionSyncNow, 'Sync now');
    });

    test('Arabic navigation labels', () async {
      final l10n = await _load(StrataLocales.arabic);
      expect(l10n.navHome, 'الرئيسية');
      expect(l10n.navDirectory, 'الدليل');
      expect(l10n.navAsk, 'اسأل');
      expect(l10n.actionNewCapture, 'التقاط جديد');
    });

    test('sync strings match the design spec copy', () async {
      final l10n = await _load(StrataLocales.english);
      expect(l10n.syncSynced(time: '14:32'), 'Synced · 14:32');
      expect(l10n.syncOfflineQueued(count: 3), 'Offline · 3 changes queued');
      expect(l10n.syncOfflineQueued(count: 1), 'Offline · 1 change queued');
      expect(l10n.syncSyncing(done: 12, total: 40), 'Syncing 12/40');
      expect(l10n.syncConflicts(count: 1), '1 conflict');
      expect(l10n.syncConflicts(count: 2), '2 conflicts');
    });

    test('Arabic plurals use the Arabic plural categories', () async {
      final l10n = await _load(StrataLocales.arabic);
      expect(l10n.syncConflicts(count: 1), 'تعارض واحد');
      expect(l10n.syncConflicts(count: 2), 'تعارضان');
      expect(l10n.syncConflicts(count: 3), '3 تعارضات');
      expect(l10n.syncConflicts(count: 11), '11 تعارضًا');
      expect(l10n.syncConflicts(count: 100), '100 تعارض');
    });
  });

  group('direction', () {
    testWidgets('Arabic UI is right-to-left', (tester) async {
      final result = await _pumpIn(tester, StrataLocales.arabic);
      expect(result.direction, TextDirection.rtl);
      expect(result.l10n.localeName, 'ar');
    });

    testWidgets('English UI is left-to-right', (tester) async {
      final result = await _pumpIn(tester, StrataLocales.english);
      expect(result.direction, TextDirection.ltr);
      expect(result.l10n.navInbox, 'Inbox');
    });
  });
}
