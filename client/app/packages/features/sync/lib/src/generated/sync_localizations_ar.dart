// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'sync_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class SyncLocalizationsAr extends SyncLocalizations {
  SyncLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String pillSemantics({required String status}) {
    return 'حالة المزامنة: $status';
  }

  @override
  String get pillOpenHint => 'افتح حالة المزامنة';

  @override
  String get bodyOnline => 'التغييرات تتزامن تلقائيًا مع الخادم.';

  @override
  String get bodyOffline =>
      'تغييراتك محفوظة على هذا الجهاز وهتتزامن لما ترجع أونلاين.';

  @override
  String get bodyUnknown => 'في انتظار أول رد من الخادم.';

  @override
  String get phaseIdle => 'خامل';

  @override
  String get phaseBootstrapping => 'جارٍ تنزيل الخزنة';

  @override
  String get phasePushing => 'جارٍ إرسال التغييرات';

  @override
  String get phasePulling => 'جارٍ سحب التغييرات';

  @override
  String get phaseBackoff => 'في انتظار إعادة المحاولة';

  @override
  String progressPages({required int done, required int total}) {
    return '$done من $total صفحة';
  }

  @override
  String progressPagesUnknown({required int done}) {
    return '$done صفحة';
  }

  @override
  String progressOps({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تغيير قيد الإرسال',
      many: '$count تغييرًا قيد الإرسال',
      few: '$count تغييرات قيد الإرسال',
      two: 'تغييران قيد الإرسال',
      one: 'تغيير واحد قيد الإرسال',
      zero: 'لا توجد تغييرات قيد الإرسال',
    );
    return '$_temp0';
  }

  @override
  String retryAt({required String time}) {
    return 'المحاولة التالية الساعة $time';
  }

  @override
  String get lastSynced => 'آخر مزامنة';

  @override
  String get lastSyncedNever => 'أبدًا';

  @override
  String get server => 'الخادم';

  @override
  String get snapshot => 'نسخة كاملة على هذا الجهاز';

  @override
  String get snapshotDone => 'نعم';

  @override
  String get snapshotPending => 'ليس بعد';

  @override
  String outboxTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تغيير في الانتظار',
      many: '$count تغييرًا في الانتظار',
      few: '$count تغييرات في الانتظار',
      two: 'تغييران في الانتظار',
      one: 'تغيير واحد في الانتظار',
      zero: 'لا شيء في الانتظار',
    );
    return '$_temp0';
  }

  @override
  String get outboxOrder => 'الأقدم أولًا';

  @override
  String get outboxEmpty => 'كل التغييرات على الجهاز ده اتزامنت.';

  @override
  String get statusPending => 'في الانتظار';

  @override
  String get statusInflight => 'جارٍ الإرسال';

  @override
  String get statusConflict => 'تعارض';

  @override
  String get statusDuplicate => 'موجود بالفعل؟';

  @override
  String attempts({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count محاولة',
      many: '$count محاولة',
      few: '$count محاولات',
      two: 'محاولتان',
      one: 'محاولة واحدة',
      zero: '',
    );
    return '$_temp0';
  }

  @override
  String get kindNoteCreate => 'ملاحظة جديدة';

  @override
  String get kindNoteUpdate => 'تعديل ملاحظة';

  @override
  String get kindNoteMove => 'نقل ملاحظة';

  @override
  String get kindNoteDelete => 'حذف ملاحظة';

  @override
  String get kindEntityCreate => 'شخص أو شركة جديدة';

  @override
  String get kindEntityPatch => 'تعديل شخص أو شركة';

  @override
  String get kindEntityMerge => 'دمج';

  @override
  String get kindDocument => 'تغيير مستند';

  @override
  String get kindPlace => 'تغيير مكان';

  @override
  String get kindRelationAdd => 'إضافة علاقة';

  @override
  String get kindRelationRemove => 'حذف علاقة';

  @override
  String get kindRelationRetype => 'تغيير نوع العلاقة';

  @override
  String get kindRelink => 'طلب إعادة الربط';

  @override
  String get kindSuggestionAccept => 'قبول اقتراح';

  @override
  String get kindSuggestionReject => 'رفض اقتراح';

  @override
  String get kindSuggestionReply => 'الرد على اقتراح';

  @override
  String get kindTaskCreate => 'مهمة جديدة';

  @override
  String get kindTaskUpdate => 'تعديل مهمة';

  @override
  String get kindTaskComplete => 'إنهاء مهمة';

  @override
  String get kindTaskCancel => 'إلغاء مهمة';

  @override
  String get kindTaskReopen => 'إعادة فتح مهمة';

  @override
  String get kindTaskDelete => 'حذف مهمة';

  @override
  String get kindDeviceSettings => 'إعدادات الجهاز';

  @override
  String get kindOther => 'تغيير';

  @override
  String conflictsTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تعارض محتاج مراجعة',
      many: '$count تعارضًا محتاج مراجعة',
      few: '$count تعارضات محتاجة مراجعة',
      two: 'تعارضان محتاجين مراجعة',
      one: 'تعارض واحد محتاج مراجعة',
    );
    return '$_temp0';
  }

  @override
  String get conflictRow => 'اتعدلت هنا وعلى الخادم';

  @override
  String get review => 'مراجعة';

  @override
  String reviewSemantics({required String title}) {
    return 'مراجعة التعارض في $title';
  }

  @override
  String get rejectionsTitle => 'تغييرات رفضها الخادم';

  @override
  String get dismiss => 'تجاهل';

  @override
  String dismissSemantics({required String message}) {
    return 'تجاهل التنبيه: $message';
  }

  @override
  String get lastError => 'آخر خطأ';

  @override
  String get retryNow => 'أعد المحاولة الآن';

  @override
  String get syncNow => 'زامن الآن';

  @override
  String get close => 'إغلاق';

  @override
  String get panelTitle => 'المزامنة';

  @override
  String get loading => 'جارٍ تحميل حالة المزامنة';

  @override
  String get loadFailed => 'تعذر تحميل حالة المزامنة';

  @override
  String get conflictBreadcrumb => 'المزامنة › التعارضات';

  @override
  String get conflictIntro =>
      'الجهاز ده والخادم عدّلوا الملاحظة دي. التغييرات اللي مش متداخلة اتدمجت؛ الأجزاء اللي اتغيرت في الناحيتين محتاجة اختيارك.';

  @override
  String get conflictCleanIntro =>
      'الجهاز ده والخادم عدّلوا الملاحظة دي. التغييرات مش متداخلة واتدمجت بدون مشاكل.';

  @override
  String get conflictGone => 'التعارض ده اتحل';

  @override
  String get conflictGoneBody => 'مفيش حاجة محتاجة قرار هنا.';

  @override
  String get columnDevice => 'الجهاز ده';

  @override
  String get columnMerged => 'النتيجة المدموجة';

  @override
  String get columnMergedEditable => 'قابلة للتعديل';

  @override
  String get columnServer => 'الخادم';

  @override
  String get columnBase => 'الأصل';

  @override
  String get versionMissing => 'غير متاح بعد';

  @override
  String get mergedFieldLabel => 'نص الملاحظة المدموج';

  @override
  String hunkTitle({required String location}) {
    return 'اتغير في الناحيتين · $location';
  }

  @override
  String get hunkOurs => 'احتفظ بنص الجهاز';

  @override
  String get hunkTheirs => 'احتفظ بنص الخادم';

  @override
  String get hunkBase => 'احتفظ بالأصل';

  @override
  String get hunkBoth => 'احتفظ بالاتنين، الجهاز ده الأول';

  @override
  String get hunkOwn => 'اكتب نصي';

  @override
  String hunkOwnField({required String location}) {
    return 'نصك لـ $location';
  }

  @override
  String hunksLeft({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count اختيار متبقي',
      many: '$count اختيارًا متبقيًا',
      few: '$count اختيارات متبقية',
      two: 'اختياران متبقيان',
      one: 'اختيار واحد متبقي',
      zero: 'كل الاختيارات اتعملت',
    );
    return '$_temp0';
  }

  @override
  String get conflictFooter =>
      'مفيش حاجة هتضيع — النسخ التانية بتفضل في السجل.';

  @override
  String get keepServer => 'احتفظ بنسخة الخادم';

  @override
  String get keepDevice => 'احتفظ بنسخة الجهاز';

  @override
  String get keepMerged => 'احتفظ بالمدموج';

  @override
  String get decideLater => 'اقفل وقرر بعدين';

  @override
  String resolveFailed({required String message}) {
    return 'تعذر حل التعارض: $message';
  }

  @override
  String get errorOffline => 'أنت غير متصل.';

  @override
  String get errorNotFound => 'لم يعد موجودًا.';

  @override
  String errorGeneric({required String code}) {
    return 'حصل خطأ ($code).';
  }

  @override
  String pillSemanticsWithHint({required String status}) {
    return 'حالة المزامنة: $status. افتح حالة المزامنة';
  }

  @override
  String get bodySyncing => 'بنبعت تغييراتك وبعدين بنجيب تغييرات السيرفر.';

  @override
  String get bodyConflict =>
      'ملاحظة اتعدلت هنا وعلى السيرفر. راجعها عشان تكمل مزامنتها.';

  @override
  String get bodyDuplicates => 'فيه تغيير شبه حاجة موجودة. اختار تحتفظ بإيه.';

  @override
  String get bodyPaused =>
      'المزامنة واقفة. تغييراتك هتفضل على الجهاز ده لحد ما تكمّل.';

  @override
  String get bodyError => 'آخر مزامنة فشلت. هتتعاد لوحدها.';

  @override
  String progressItems({required int done, required int total}) {
    return '$done من $total';
  }

  @override
  String pulledCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'اتجاب $count تغيير',
      few: 'اتجاب $count تغييرات',
      two: 'اتجاب تغييرين',
      one: 'اتجاب تغيير واحد',
      zero: 'لسه مفيش حاجة اتجابت',
    );
    return '$_temp0';
  }

  @override
  String get pauseSync => 'وقّف المزامنة';

  @override
  String get resumeSync => 'كمّل المزامنة';

  @override
  String get syncLog => 'سجل المزامنة';

  @override
  String get syncLogEmpty => 'لسه مفيش حاجة في السجل.';

  @override
  String get saveBothCopies => 'احفظ الاتنين كنسخ';

  @override
  String conflictCopySaved({required String path}) {
    return 'اتحفظت نسخة في $path';
  }

  @override
  String get legendAdded => 'اتضاف';

  @override
  String get legendRemoved => 'اتشال';

  @override
  String get legendChanged => 'اتغير في الناحيتين';

  @override
  String lineSemantics({
    required int number,
    required String change,
    required String text,
  }) {
    return 'سطر $number، $change: $text';
  }

  @override
  String get lineSame => 'زي ما هو';

  @override
  String get lineChangedOneSide => 'اتغير';

  @override
  String get hunkBothServerFirst => 'احتفظ بالاتنين، السيرفر الأول';
}
