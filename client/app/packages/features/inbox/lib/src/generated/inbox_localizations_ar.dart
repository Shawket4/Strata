// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'inbox_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class InboxLocalizationsAr extends InboxLocalizations {
  InboxLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get inboxTitle => 'الوارد';

  @override
  String get inboxLoadError => 'تعذّر تحميل الوارد';

  @override
  String get inboxEmptyTitle => 'الوارد فارغ';

  @override
  String get inboxEmptyMessage =>
      'ما تحفظه من أفكار يصل هنا، ويقترح الذكاء الاصطناعي مكانه. لا يُحفظ شيء قبل موافقتك.';

  @override
  String get inboxAccept => 'قبول';

  @override
  String get inboxReject => 'رفض';

  @override
  String get inboxEdit => 'تعديل';

  @override
  String get inboxCancel => 'إلغاء';

  @override
  String get inboxUndo => 'تراجع';

  @override
  String get inboxDiscard => 'تجاهل';

  @override
  String get inboxDismiss => 'إخفاء';

  @override
  String get inboxActionFailed => 'تعذّر تطبيق هذا التغيير';

  @override
  String inboxActionFailedCode({required String code}) {
    return 'تعذّر تطبيق هذا التغيير ($code)';
  }

  @override
  String inboxAiConfidence({required String score}) {
    return 'ذكاء اصطناعي · $score';
  }

  @override
  String get inboxAiProposal => 'اقتراح الذكاء الاصطناعي';

  @override
  String get inboxNoProposalYet =>
      'لا اقتراح بعد — لم يقرأ الذكاء الاصطناعي هذه الفكرة.';

  @override
  String inboxProposalCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count اقتراح',
      many: '$count اقتراحًا',
      few: '$count اقتراحات',
      two: 'اقتراحان',
      one: 'اقتراح واحد',
      zero: 'لا اقتراحات',
    );
    return '$_temp0';
  }

  @override
  String inboxTag({required String tag}) {
    return '#$tag';
  }

  @override
  String get inboxPeopleAndCompanies => 'الأشخاص والشركات';

  @override
  String inboxCreatesNoteIn({required String folder}) {
    return 'القبول يحفظها كملاحظة جديدة في $folder';
  }

  @override
  String get inboxRejectRelation => 'رفض العلاقة';

  @override
  String get inboxTaskProposal => 'مهمة مقترحة';

  @override
  String get inboxNeedsYou => 'يحتاجك';

  @override
  String get inboxAppliedAutomatically => 'طُبّق تلقائيًا';

  @override
  String get inboxCustodySuggestion => 'يحتاجك · حيازة';

  @override
  String get inboxPossibleDuplicate => 'تكرار محتمل';

  @override
  String get inboxRelationSuggestion => 'اقتراح علاقة';

  @override
  String get inboxTaskSuggestion => 'اقتراح مهمة';

  @override
  String get inboxFilingProposal => 'اقتراح حفظ';

  @override
  String get inboxUnsupportedBadge => 'اقتراح';

  @override
  String inboxUnsupported({required String kind}) {
    return 'هذا الإصدار لا يعرض اقتراحات «$kind» بعد. حدّث التطبيق لمراجعته.';
  }

  @override
  String get inboxRejected => 'مرفوض';

  @override
  String inboxWhoIs({required String mention}) {
    return 'مين «$mention»؟';
  }

  @override
  String get inboxNoPersonMatches =>
      'اسم تدليل. لا يوجد شخص في خزنتك يطابقه بعد.';

  @override
  String get inboxPossibleMatches => 'تطابقات محتملة في خزنتك:';

  @override
  String get inboxCreatePerson => 'إنشاء شخص…';

  @override
  String inboxAliasNote({required String mention}) {
    return 'القبول يضيف «$mention» كاسم بديل، لتُفهم الإشارات القادمة.';
  }

  @override
  String get inboxCreatePersonTitle => 'إنشاء شخص';

  @override
  String get inboxPersonName => 'الاسم';

  @override
  String get inboxAliases => 'الأسماء البديلة';

  @override
  String get inboxCreatePersonSave => 'إنشاء';

  @override
  String get inboxWhichDocument => 'أي مستند؟';

  @override
  String get inboxCaptureSemantics => 'فكرة ملتقطة';

  @override
  String get inboxBulkActions => 'إجراءات جماعية';

  @override
  String get inboxSelectAll => 'تحديد كل الأفكار';

  @override
  String inboxSelectedCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count محددة',
      many: '$count محددة',
      few: '$count محددة',
      two: 'اثنتان محددتان',
      one: 'واحدة محددة',
      zero: 'لا شيء محدد',
    );
    return '$_temp0';
  }

  @override
  String get inboxClearSelection => 'مسح التحديد';

  @override
  String get inboxSelectCapture => 'تحديد الفكرة';

  @override
  String get inboxSuggestionsHeader => 'الاقتراحات';

  @override
  String get inboxCapturesHeader => 'الأفكار الملتقطة';

  @override
  String get inboxAlsoNeedsYou => 'يحتاجك أيضًا';

  @override
  String get inboxCaptureHeader => 'الفكرة';

  @override
  String inboxRelationSummary({required String type, required String target}) {
    return '$type $target';
  }

  @override
  String get inboxShortcutsLabel => 'اختصارات لوحة المفاتيح';

  @override
  String get inboxKeyMove => 'تنقّل';

  @override
  String get inboxKeySelect => 'تحديد';

  @override
  String get inboxKeyAccept => 'قبول';

  @override
  String get inboxKeyReject => 'رفض';

  @override
  String get inboxKeyEdit => 'تعديل';

  @override
  String get inboxReady => 'جاهز';

  @override
  String get inboxLooksRight => 'يبدو صحيحًا';

  @override
  String get inboxKeepBoth => 'احتفظ بالاثنين';

  @override
  String inboxYouSaid({required String words}) {
    return 'قلت: \"$words\"';
  }

  @override
  String get inboxNickname => 'لقب';

  @override
  String inboxItIs({required String title}) {
    return 'إنه $title';
  }

  @override
  String get inboxCreateCompany => 'إنشاء شركة…';

  @override
  String inboxQuote({required String quote}) {
    return '\"$quote\"';
  }

  @override
  String inboxAfterAt({required String place}) {
    return 'بعدها في $place';
  }

  @override
  String inboxAfterWith({required String person}) {
    return 'بعدها مع $person';
  }

  @override
  String inboxAfterLastWith({required String person}) {
    return 'لا أحد معه · آخر مرة مع $person';
  }

  @override
  String inboxThreadAi({required String when}) {
    return 'الذكاء الاصطناعي · $when';
  }

  @override
  String inboxThreadYou({required String when}) {
    return 'أنت · $when';
  }

  @override
  String get inboxReply => 'رد على الذكاء الاصطناعي';

  @override
  String get inboxReplyField => 'ردك';

  @override
  String get inboxReplySend => 'إرسال الرد';

  @override
  String get inboxEditTitle => 'عدّل قبل القبول';

  @override
  String get inboxEditTaskText => 'المهمة';

  @override
  String get inboxEditNoteTitle => 'عنوان الملاحظة';

  @override
  String get inboxEditFolder => 'المجلد';

  @override
  String get inboxAcceptEdited => 'قبول مع التعديلات';

  @override
  String inboxFilterAll({required int count}) {
    return 'الكل · $count';
  }

  @override
  String inboxFilterNeedsYou({required int count}) {
    return 'يحتاجك · $count';
  }

  @override
  String inboxFilterConflicts({required int count}) {
    return 'تعارضات · $count';
  }

  @override
  String get inboxFilterLabel => 'عرض';

  @override
  String inboxAcceptAllReady({required int count}) {
    return 'قبول كل الجاهز · $count';
  }
}
