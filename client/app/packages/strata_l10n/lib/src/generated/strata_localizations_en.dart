// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'strata_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class StrataLocalizationsEn extends StrataLocalizations {
  StrataLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'Strata';

  @override
  String get navigationLabel => 'Main navigation';

  @override
  String get navHome => 'Home';

  @override
  String get navInbox => 'Inbox';

  @override
  String get navTasks => 'Tasks';

  @override
  String get navNotes => 'Notes';

  @override
  String get navMap => 'Map';

  @override
  String get navDirectory => 'Directory';

  @override
  String get navAsk => 'Ask';

  @override
  String get navSettings => 'Settings';

  @override
  String get actionNewCapture => 'New capture';

  @override
  String get actionCapture => 'Capture';

  @override
  String get actionSearch => 'Search';

  @override
  String get actionSave => 'Save';

  @override
  String get actionCancel => 'Cancel';

  @override
  String get actionAccept => 'Accept';

  @override
  String get actionReject => 'Reject';

  @override
  String get actionEdit => 'Edit';

  @override
  String get actionDelete => 'Delete';

  @override
  String get actionOpen => 'Open';

  @override
  String get actionBack => 'Back';

  @override
  String get actionClose => 'Close';

  @override
  String get actionRetry => 'Retry';

  @override
  String get actionSyncNow => 'Sync now';

  @override
  String get actionSignOut => 'Sign out';

  @override
  String get actionSaveAsNote => 'Save as note';

  @override
  String get contextPanelLabel => 'Context';

  @override
  String syncSynced({required String time}) {
    return 'Synced · $time';
  }

  @override
  String syncOfflineQueued({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Offline · $count changes queued',
      one: 'Offline · 1 change queued',
      zero: 'Offline',
    );
    return '$_temp0';
  }

  @override
  String syncSyncing({required int done, required int total}) {
    return 'Syncing $done/$total';
  }

  @override
  String syncConflicts({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count conflicts',
      one: '1 conflict',
    );
    return '$_temp0';
  }

  @override
  String get syncStatusLabel => 'Sync status';

  @override
  String get relationRelated => 'related';

  @override
  String get relationPartOf => 'part of';

  @override
  String get relationSupports => 'supports';

  @override
  String get relationContradicts => 'contradicts';

  @override
  String get relationFollowsUp => 'follows up';

  @override
  String get relationDuplicates => 'duplicates';

  @override
  String get relationSimilarity => 'similar';

  @override
  String get relationBodyLink => 'link';

  @override
  String get relationMention => 'mentions';

  @override
  String get aiTag => 'AI';

  @override
  String aiConfidenceSemantics({required String confidence}) {
    return 'Suggested by AI, confidence $confidence';
  }

  @override
  String get nodeKindNote => 'Note';

  @override
  String get nodeKindConcept => 'Concept';

  @override
  String get nodeKindPerson => 'Person';

  @override
  String get nodeKindCompany => 'Company';

  @override
  String get nodeKindDocument => 'Document';

  @override
  String get nodeKindPlace => 'Place';

  @override
  String citationSemantics({required String label}) {
    return 'Source: $label';
  }

  @override
  String shortcutSemantics({required String keys}) {
    return 'Keyboard shortcut $keys';
  }

  @override
  String get featurePlaceholderMessage => 'This screen is being built.';

  @override
  String get featureEditor => 'Note editor';

  @override
  String get featureDocuments => 'Documents';

  @override
  String get featureSync => 'Sync & conflicts';

  @override
  String get featureAccounts => 'Sign in';

  @override
  String get featureAdmin => 'Users';

  @override
  String get errorPageNotFound => 'Page not found';
}
