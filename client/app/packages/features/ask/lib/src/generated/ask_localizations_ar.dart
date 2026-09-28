// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'ask_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class AskLocalizationsAr extends AskLocalizations {
  AskLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get askTitle => 'اسأل';

  @override
  String get conversation => 'المحادثة';

  @override
  String get newConversation => 'محادثة جديدة';

  @override
  String get offlineTitle => 'خاصية اسأل محتاجة اتصال';

  @override
  String get offlineMessage =>
      'ملاحظاتك لسه ممكن تدوّر فيها من غير نت. اسأل هترجع تشتغل لما ترجع أونلاين.';

  @override
  String get notYetTitle => 'اسأل مش متاحة على السيرفر ده لسه';

  @override
  String get notYetMessage =>
      'الإجابات مع المصادر هتظهر هنا لما السيرفر يدعم اسأل.';

  @override
  String get notAllowedTitle => 'اسأل مش متاحة للحساب ده';

  @override
  String get emptyTitle => 'اسأل عن ملاحظاتك';

  @override
  String get emptyMessage =>
      'الإجابات بتذكر الملاحظات اللي جت منها، بالعربي أو بالإنجليزي.';

  @override
  String get you => 'إنت';

  @override
  String get answer => 'الإجابة';

  @override
  String sources({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'المصادر · $count',
      one: 'المصادر · 1',
      zero: 'مفيش مصادر',
    );
    return '$_temp0';
  }

  @override
  String get saveAsNote => 'احفظ كملاحظة';

  @override
  String get copyAnswer => 'انسخ الإجابة';

  @override
  String get copied => 'اتنسخت الإجابة';

  @override
  String get question => 'السؤال';

  @override
  String get askHint => 'اسأل عن ملاحظاتك — بالعربي أو English';

  @override
  String get send => 'إرسال';

  @override
  String get scope => 'النطاق';

  @override
  String get sourcePreview => 'معاينة المصدر';

  @override
  String get sourcePreviewHint => 'اختار مصدر علشان تعاين ملاحظته';

  @override
  String get openAtBlock => 'افتح عند الفقرة';

  @override
  String get noteMissing => 'الملاحظة المذكورة مش على الجهاز ده';

  @override
  String get searchTitle => 'بحث';

  @override
  String get searchField => 'ابحث في ملاحظاتك';

  @override
  String get searchHint => 'ابحث بالعربي أو English';

  @override
  String get searchMode => 'نوع البحث';

  @override
  String get modeKeyword => 'كلمات';

  @override
  String get modeSemantic => 'بالمعنى';

  @override
  String get modeHybrid => 'مختلط';

  @override
  String get searchPrompt => 'اكتب علشان تدوّر';

  @override
  String get searchPromptMessage =>
      'البحث بالكلمات بيشتغل من غير نت؛ البحث بالمعنى والمختلط محتاجين السيرفر.';

  @override
  String noResults({required String query}) {
    return 'مفيش نتايج لـ “$query”';
  }

  @override
  String get noResultsMessage => 'جرّب كلمات تانية، أو باللغة التانية.';

  @override
  String modeOffline({required String mode}) {
    return 'البحث $mode محتاج اتصال — مفيش نتايج لحد ما ترجع أونلاين.';
  }

  @override
  String modeNotYet({required String mode}) {
    return 'البحث $mode مش متاح على السيرفر ده لسه.';
  }

  @override
  String modeNotAllowed({required String mode}) {
    return 'البحث $mode مش متاح للحساب ده.';
  }

  @override
  String resultsCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count نتايج',
      one: 'نتيجة واحدة',
    );
    return '$_temp0';
  }

  @override
  String get openNote => 'افتح الملاحظة';

  @override
  String get preview => 'معاينة';

  @override
  String get previewHint => 'اختار نتيجة علشان تعاينها';

  @override
  String get loading => 'جارِ التحميل…';

  @override
  String get errorTitle => 'تعذّر التحميل';

  @override
  String errorMessage({required String code}) {
    return 'البيانات المحلية للتطبيق رجّعت خطأ ($code).';
  }

  @override
  String get stop => 'إيقاف';

  @override
  String get answering => 'جارٍ الإجابة…';

  @override
  String get answerStopped => 'أوقفت هذه الإجابة.';

  @override
  String get answerPaused =>
      'الذكاء الاصطناعي متوقف مؤقتًا — توقفت الإجابة مبكرًا.';

  @override
  String get answerUnavailable =>
      'لا يمكن الوصول للذكاء الاصطناعي — توقفت الإجابة مبكرًا.';

  @override
  String get answerFailed => 'توقفت هذه الإجابة مبكرًا.';

  @override
  String get savedAsNote => 'تم الحفظ كملاحظة';

  @override
  String get openSavedNote => 'محفوظة · افتح الملاحظة';

  @override
  String citationMarker({required int index}) {
    return 'الاستشهاد $index';
  }
}
