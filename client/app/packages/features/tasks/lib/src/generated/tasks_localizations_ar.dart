// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'tasks_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class TasksLocalizationsAr extends TasksLocalizations {
  TasksLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get commonCancel => 'إلغاء';

  @override
  String get commonSave => 'حفظ';

  @override
  String get commonLoading => 'جارٍ التحميل';

  @override
  String commonCoreFailure({required String code}) {
    return 'أبلغ التطبيق عن: $code';
  }

  @override
  String get commonUnknownFailure => 'حدث خطأ على هذا الجهاز.';

  @override
  String get commonIntentFailed => 'تعذّر تطبيق هذا التغيير';

  @override
  String commonIntentFailedCode({required String code}) {
    return 'تعذّر تطبيق هذا التغيير ($code)';
  }

  @override
  String get commonNone => '—';

  @override
  String get commonNotSynced => 'لم تتم المزامنة بعد';

  @override
  String get commonNotYetAvailable => 'غير متاح بعد';

  @override
  String get dupEyebrow => 'موجود بالفعل';

  @override
  String get dupTitle => 'يبدو أن لديك هذا بالفعل';

  @override
  String dupSubtitle({required String kind, required String title}) {
    return 'يوجد $kind مشابه يغطي «$title».';
  }

  @override
  String get dupOpenExisting => 'فتح الموجود';

  @override
  String get dupCreateAnyway => 'إنشاء على أي حال';

  @override
  String get dupCancel => 'إلغاء';

  @override
  String get dupFootnote =>
      '«إنشاء على أي حال» يحتفظ بالاثنين ولن يسأل عن هذا الزوج مرة أخرى.';

  @override
  String dupMatch({required String level, required String score}) {
    return '$level · تطابق $score';
  }

  @override
  String get dupMatchExact => 'مطابق';

  @override
  String get dupMatchNear => 'قريب';

  @override
  String get dupMatchSemantic => 'دلالي';

  @override
  String dupCandidateSemantics({
    required String title,
    required String kind,
    required String match,
  }) {
    return 'فتح الموجود: $title، $kind، $match';
  }

  @override
  String get dupLoadError => 'تعذّر تحميل فحص التكرار';

  @override
  String get dupNoneOpen => 'لا يوجد ما ينتظر فحص التكرار.';

  @override
  String get kindTask => 'مهمة';

  @override
  String get kindNote => 'ملاحظة';

  @override
  String get kindPerson => 'شخص';

  @override
  String get kindCompany => 'شركة';

  @override
  String get kindConcept => 'مفهوم';

  @override
  String get kindDocument => 'مستند';

  @override
  String get kindPlace => 'مكان';

  @override
  String get kindItem => 'عنصر';

  @override
  String get editorNewTaskTitle => 'مهمة جديدة';

  @override
  String get editorTextLabel => 'ما المطلوب؟';

  @override
  String get editorTextHint => 'مثلًا: أرسل عرض الفوترة الأسبوعية لأحمد';

  @override
  String get editorParsedTitle => 'فُهمت على أنها';

  @override
  String get editorParsedUnavailable =>
      'قراءة المواعيد والتكرار من النص غير متاحة بعد';

  @override
  String get editorDue => 'الاستحقاق';

  @override
  String get editorPickDate => 'اختر تاريخًا';

  @override
  String get editorRepeat => 'التكرار (القاعدة كما تُكتب)';

  @override
  String get editorReminders => 'التذكيرات';

  @override
  String get editorAddReminder => 'إضافة تذكير';

  @override
  String editorReminderAt({required DateTime at}) {
    final intl.DateFormat atDateFormat = intl.DateFormat(
      'EEE d MMM HH:mm',
      localeName,
    );
    final String atString = atDateFormat.format(at);

    return '$atString';
  }

  @override
  String editorRemoveReminder({required String time}) {
    return 'إزالة التذكير $time';
  }

  @override
  String get editorHomeNote => 'الملاحظة الأم';

  @override
  String get editorDefaultHome => 'قائمة المهام الافتراضية';

  @override
  String get editorClose => 'إغلاق';

  @override
  String get editorSaveFailed => 'تعذّر حفظ المهمة';

  @override
  String get recurrenceTitle => 'التكرار';

  @override
  String get recurrenceClose => 'إغلاق محرر التكرار';

  @override
  String get recurrencePhraseLabel => 'القاعدة كما تُكتب';

  @override
  String get recurrencePhraseHint => 'every month on the 1st';

  @override
  String get recurrencePhraseHelper =>
      'تُحفظ كما هي في الملاحظة بلغة Obsidian Tasks.';

  @override
  String get recurrenceFrequency => 'التواتر';

  @override
  String get recurrenceDaily => 'يوميًا';

  @override
  String get recurrenceWeekly => 'أسبوعيًا';

  @override
  String get recurrenceMonthly => 'شهريًا';

  @override
  String get recurrenceYearly => 'سنويًا';

  @override
  String get recurrenceOnDayOfMonth => 'في يوم من الشهر';

  @override
  String get recurrenceOnNthWeekday => 'في يوم الأسبوع رقم n';

  @override
  String get recurrenceOnLastDay => 'في آخر يوم';

  @override
  String get recurrenceEndsNever => 'لا ينتهي';

  @override
  String get recurrenceEndsOnDate => 'ينتهي في تاريخ';

  @override
  String get recurrenceEndsAfter => 'ينتهي بعد N مرات';

  @override
  String get recurrenceBuilderUnavailable =>
      'بناء القاعدة من الخيارات غير متاح بعد';

  @override
  String get recurrencePreviewTitle => 'المواعيد التالية';

  @override
  String get recurrencePreviewUnavailable =>
      'معاينة المواعيد التالية غير متاحة بعد.';

  @override
  String get recurrenceSave => 'حفظ القاعدة';

  @override
  String get recurrenceStop => 'إيقاف التكرار';

  @override
  String get tasksTitle => 'المهام';

  @override
  String get tasksNewTask => 'مهمة جديدة';

  @override
  String get tasksTabToday => 'اليوم';

  @override
  String get tasksTabUpcoming => 'القادمة';

  @override
  String get tasksTabOverdue => 'المتأخرة';

  @override
  String get tasksTabRecurring => 'المتكررة';

  @override
  String get tasksTabNoDate => 'بلا تاريخ';

  @override
  String get tasksTabDone => 'المنجزة';

  @override
  String get tasksTabOpen => 'المفتوحة';

  @override
  String tasksTabWithCount({required String label, required int count}) {
    return '$label · $count';
  }

  @override
  String get tasksEmptyTitle => 'لا شيء هنا';

  @override
  String get tasksEmptyMessage =>
      'المهام بنود قوائم تحقق داخل ملاحظاتك. أضف واحدة من «مهمة جديدة».';

  @override
  String get tasksLoadError => 'تعذّر تحميل المهام';

  @override
  String get tasksDetailLoadError => 'تعذّر تحميل هذه المهمة';

  @override
  String get tasksDetailTitle => 'المهمة';

  @override
  String get tasksDetailPanelLabel => 'تفاصيل المهمة';

  @override
  String get tasksSelectPrompt => 'اختر مهمة لعرض تفاصيلها';

  @override
  String get tasksNotFoundTitle => 'المهمة غير موجودة';

  @override
  String get tasksNotFoundMessage => 'ربما حُذفت أو نُقلت من جهاز آخر.';

  @override
  String tasksDueOn({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'تستحق $dateString';
  }

  @override
  String tasksOverdueSince({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'متأخرة · كانت تستحق $dateString';
  }

  @override
  String tasksDateShort({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String tasksDateLong({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String tasksDoneOn({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat('d MMM', localeName);
    final String dateString = dateDateFormat.format(date);

    return 'أُنجزت $dateString';
  }

  @override
  String tasksHistoryDue({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'd MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'كانت تستحق $dateString';
  }

  @override
  String tasksRuleNext({required String rule, required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$rule · التالي $dateString';
  }

  @override
  String get tasksCancelled => 'ملغاة';

  @override
  String get tasksStateDone => 'منجزة';

  @override
  String get tasksRecurrenceNotUnderstood => 'لم تُفهم قاعدة التكرار';

  @override
  String tasksReminderSemantics({required String time}) {
    return 'تذكير $time';
  }

  @override
  String tasksMarkDoneSemantics({required String title}) {
    return 'تحديد كمنجزة: $title';
  }

  @override
  String tasksReopenSemantics({required String title}) {
    return 'إعادة فتح: $title';
  }

  @override
  String get tasksActionMarkDone => 'تحديد كمنجزة';

  @override
  String get tasksActionReopen => 'إعادة فتح';

  @override
  String get tasksActionCancelTask => 'إلغاء المهمة';

  @override
  String get tasksActionEditRule => 'تعديل القاعدة';

  @override
  String get tasksActionChangeDue => 'تغيير تاريخ الاستحقاق';

  @override
  String get tasksActionClearDue => 'إزالة تاريخ الاستحقاق';

  @override
  String get tasksActionAddReminder => 'إضافة تذكير';

  @override
  String get tasksActionOpenInNote => 'فتح في الملاحظة';

  @override
  String get tasksActionEditText => 'تعديل النص';

  @override
  String get tasksActionCloseDetail => 'إغلاق التفاصيل';

  @override
  String get tasksRecurring => 'متكررة';

  @override
  String get tasksOneOff => 'لمرة واحدة';

  @override
  String get tasksFieldRepeat => 'التكرار';

  @override
  String get tasksFieldDue => 'الاستحقاق';

  @override
  String get tasksFieldReminders => 'التذكيرات';

  @override
  String get tasksFieldLinked => 'مرتبطة بـ';

  @override
  String get tasksFieldHomeNote => 'الملاحظة الأم';

  @override
  String get tasksFieldStoredLine => 'السطر المحفوظ';

  @override
  String get tasksDoesNotRepeat => 'لا تتكرر';

  @override
  String get tasksNoDueDate => 'بلا تاريخ استحقاق';

  @override
  String get tasksNoReminders => 'لا تذكيرات';

  @override
  String get tasksNoLinks => 'لا روابط';

  @override
  String get tasksHistoryTitle => 'السجل';

  @override
  String get tasksHistorySubtitle => 'المرات المنجزة';

  @override
  String get tasksHistoryEmpty => 'لا توجد مرات منجزة بعد';

  @override
  String get tasksNoSkipping =>
      'لا يمكن تخطي المرات: المتأخرة تبقى مفتوحة حتى تُنجز أو تُلغى.';

  @override
  String get tasksShortcutsLabel => 'اختصارات لوحة المفاتيح';

  @override
  String get tasksShortcutMove => 'تنقّل';

  @override
  String get tasksShortcutDone => 'إنجاز';

  @override
  String get tasksShortcutRepeat => 'تكرار';

  @override
  String get tasksShortcutDue => 'الاستحقاق';

  @override
  String get tasksShortcutOpen => 'فتح في الملاحظة';

  @override
  String get tasksColumnTask => 'المهمة';

  @override
  String get tasksColumnLinked => 'مرتبطة بـ';

  @override
  String get tasksColumnRepeat => 'التكرار';

  @override
  String get tasksColumnRemind => 'التذكير';

  @override
  String get tasksColumnDue => 'الاستحقاق';

  @override
  String get tasksFooterNote =>
      'المهام أسطر Obsidian Tasks مجمّعة من ملاحظاتك. التعديل هنا يعيد كتابة السطر في ملاحظته الأم.';

  @override
  String dupSubtitleUntitled({required String kind}) {
    return 'يوجد $kind مشابه بالفعل.';
  }
}
