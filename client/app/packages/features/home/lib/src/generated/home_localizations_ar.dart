// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'home_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class HomeLocalizationsAr extends HomeLocalizations {
  HomeLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get homeTitle => 'الرئيسية';

  @override
  String get homeLoadError => 'تعذّر تحميل الرئيسية';

  @override
  String get homeComposerLabel => 'التقط فكرة';

  @override
  String get homeComposerHint => 'اكتب أو أملِ · بالعربية أو الإنجليزية';

  @override
  String get homeComposerFooter =>
      'يرتّبها الذكاء الاصطناعي في الوارد؛ لا يُحفظ شيء قبل موافقتك.';

  @override
  String get homeComposerOffline =>
      'غير متصل · تُحفظ الأفكار على هذا الجهاز وتُزامن لاحقًا.';

  @override
  String get homeFocusHint => 'التقاط';

  @override
  String get homeSave => 'حفظ';

  @override
  String get homeCaptureSaved => 'حُفظت في الوارد';

  @override
  String get homeCaptureFailed => 'تعذّر حفظ الفكرة';

  @override
  String homeCaptureFailedCode({required String code}) {
    return 'تعذّر حفظ الفكرة ($code)';
  }

  @override
  String homeInboxWaiting({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count فكرة تنتظر الترتيب',
      many: '$count فكرة تنتظر الترتيب',
      few: '$count أفكار تنتظر الترتيب',
      two: 'فكرتان تنتظران الترتيب',
      one: 'فكرة واحدة تنتظر الترتيب',
      zero: 'الوارد فارغ',
    );
    return '$_temp0';
  }

  @override
  String get homeAllTasks => 'كل المهام';

  @override
  String get homeAllNotes => 'كل الملاحظات';

  @override
  String get homeNoOpenTasks =>
      'لا مهام مفتوحة. أضف واحدة من فكرة أو من «مهمة جديدة».';

  @override
  String get homeNothingToday => 'لا شيء مستحق اليوم.';

  @override
  String get homeRecentNotes => 'أحدث الملاحظات';

  @override
  String get homeNoNotes => 'لا ملاحظات بعد. أول فكرة تصبح ملاحظة بعد قبولها.';

  @override
  String get homeColumnNote => 'الملاحظة';

  @override
  String get homeColumnPath => 'المسار';

  @override
  String get homeColumnEdited => 'آخر تعديل';

  @override
  String get homeSecondaryLabel => 'اليوم والوارد ونشاط الذكاء الاصطناعي';

  @override
  String get homeAiActivity => 'نشاط الذكاء الاصطناعي';

  @override
  String get homeAiActivityUnavailable =>
      'العلاقات التي يضيفها الذكاء الاصطناعي والتناقضات التي يجدها والتغييرات التلقائية التي يمكنك التراجع عنها تظهر هنا عند الاتصال بالخادم.';

  @override
  String get homeOpenItems => 'بنود مفتوحة';

  @override
  String get homeOpenItemsUnavailable =>
      'ستُجمع هنا البنود المفتوحة من صفحات الأشخاص والشركات.';

  @override
  String get homeNotYetAvailable => 'غير متاح بعد';

  @override
  String homeNeedsYou({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تحتاجك',
      many: '$count تحتاجك',
      few: '$count تحتاجك',
      two: 'اثنتان تحتاجانك',
      one: 'واحدة تحتاجك',
    );
    return '$_temp0';
  }

  @override
  String homeContradictions({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تناقض',
      many: '$count تناقضًا',
      few: '$count تناقضات',
      two: 'تناقضان',
      one: 'تناقض واحد',
    );
    return '$_temp0';
  }

  @override
  String homeLinks({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count رابط',
      many: '$count رابطًا',
      few: '$count روابط',
      two: 'رابطان',
      one: 'رابط واحد',
    );
    return '$_temp0';
  }

  @override
  String get homeRecentFilter => 'عرض';

  @override
  String get homeRecentEdited => 'المعدّلة';

  @override
  String get homeRecentCreated => 'الجديدة';

  @override
  String get homeRecentFiledByAi => 'صنّفها الذكاء الاصطناعي';

  @override
  String homeActionFailed({required String code}) {
    return 'لم يتم ذلك ($code).';
  }

  @override
  String get homeOffline => 'غير متصل';

  @override
  String get homeNotAllowed => 'غير متاح لهذا الحساب';

  @override
  String get homeRetypeTitle => 'تغيير نوع العلاقة';

  @override
  String homeAiConfidence({required String value}) {
    return 'ذكاء اصطناعي · $value';
  }

  @override
  String get homeAiUndone => 'تم التراجع';

  @override
  String get homeAiRetype => 'غيّر النوع';

  @override
  String get homeAiUndo => 'تراجع';

  @override
  String get homeAiActivityEmpty => 'لا جديد من الذكاء الاصطناعي.';

  @override
  String get homeOpenItemsEmpty => 'لا توجد بنود مفتوحة.';
}
