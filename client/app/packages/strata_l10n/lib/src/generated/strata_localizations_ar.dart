// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'strata_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class StrataLocalizationsAr extends StrataLocalizations {
  StrataLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get appTitle => 'ستراتا';

  @override
  String get navigationLabel => 'التنقل الرئيسي';

  @override
  String get navHome => 'الرئيسية';

  @override
  String get navInbox => 'الوارد';

  @override
  String get navTasks => 'المهام';

  @override
  String get navNotes => 'الملاحظات';

  @override
  String get navMap => 'الخريطة';

  @override
  String get navDirectory => 'الدليل';

  @override
  String get navAsk => 'اسأل';

  @override
  String get navSettings => 'الإعدادات';

  @override
  String get actionNewCapture => 'التقاط جديد';

  @override
  String get actionCapture => 'التقاط';

  @override
  String get actionSearch => 'بحث';

  @override
  String get actionSave => 'حفظ';

  @override
  String get actionCancel => 'إلغاء';

  @override
  String get actionAccept => 'قبول';

  @override
  String get actionReject => 'رفض';

  @override
  String get actionEdit => 'تعديل';

  @override
  String get actionDelete => 'حذف';

  @override
  String get actionOpen => 'فتح';

  @override
  String get actionBack => 'رجوع';

  @override
  String get actionClose => 'إغلاق';

  @override
  String get actionRetry => 'إعادة المحاولة';

  @override
  String get actionSyncNow => 'زامن الآن';

  @override
  String get actionSignOut => 'تسجيل الخروج';

  @override
  String get actionSaveAsNote => 'حفظ كملاحظة';

  @override
  String get contextPanelLabel => 'السياق';

  @override
  String syncSynced({required String time}) {
    return 'متزامن · $time';
  }

  @override
  String syncOfflineQueued({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'غير متصل · $count تغيير في الانتظار',
      many: 'غير متصل · $count تغييرًا في الانتظار',
      few: 'غير متصل · $count تغييرات في الانتظار',
      two: 'غير متصل · تغييران في الانتظار',
      one: 'غير متصل · تغيير واحد في الانتظار',
      zero: 'غير متصل',
    );
    return '$_temp0';
  }

  @override
  String syncSyncing({required int done, required int total}) {
    return 'جارٍ المزامنة $done/$total';
  }

  @override
  String syncConflicts({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تعارض',
      many: '$count تعارضًا',
      few: '$count تعارضات',
      two: 'تعارضان',
      one: 'تعارض واحد',
    );
    return '$_temp0';
  }

  @override
  String get syncStatusLabel => 'حالة المزامنة';

  @override
  String get relationRelated => 'مرتبط';

  @override
  String get relationPartOf => 'جزء من';

  @override
  String get relationSupports => 'يدعم';

  @override
  String get relationContradicts => 'يناقض';

  @override
  String get relationFollowsUp => 'متابعة لـ';

  @override
  String get relationDuplicates => 'مكرر';

  @override
  String get relationSimilarity => 'مشابه';

  @override
  String get relationBodyLink => 'رابط';

  @override
  String get relationMention => 'يذكر';

  @override
  String get aiTag => 'ذكاء اصطناعي';

  @override
  String aiConfidenceSemantics({required String confidence}) {
    return 'مقترح من الذكاء الاصطناعي، الثقة $confidence';
  }

  @override
  String get nodeKindNote => 'ملاحظة';

  @override
  String get nodeKindConcept => 'مفهوم';

  @override
  String get nodeKindPerson => 'شخص';

  @override
  String get nodeKindCompany => 'شركة';

  @override
  String get nodeKindDocument => 'مستند';

  @override
  String get nodeKindPlace => 'مكان';

  @override
  String citationSemantics({required String label}) {
    return 'المصدر: $label';
  }

  @override
  String shortcutSemantics({required String keys}) {
    return 'اختصار لوحة المفاتيح $keys';
  }

  @override
  String get featurePlaceholderMessage => 'هذه الشاشة قيد الإنشاء.';

  @override
  String get featureEditor => 'محرر الملاحظات';

  @override
  String get featureDocuments => 'المستندات';

  @override
  String get featureSync => 'المزامنة والتعارضات';

  @override
  String get featureAccounts => 'تسجيل الدخول';

  @override
  String get featureAdmin => 'المستخدمون';
}
