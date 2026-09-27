// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'notes_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class NotesLocalizationsAr extends NotesLocalizations {
  NotesLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get notesRoot => 'الملاحظات';

  @override
  String get folderBreadcrumbLabel => 'المجلد';

  @override
  String folderNoteCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ملاحظة',
      many: '$count ملاحظة',
      few: '$count ملاحظات',
      two: 'ملاحظتين',
      one: 'ملاحظة واحدة',
      zero: 'مفيش ملاحظات',
    );
    return '$_temp0';
  }

  @override
  String folderRowSemantics({required String name, required String count}) {
    return 'مجلد $name، $count';
  }

  @override
  String get searchNotesLabel => 'دور في الملاحظات';

  @override
  String get clearSearch => 'مسح البحث';

  @override
  String notesInFolderLabel({required String folder}) {
    return 'الملاحظات في $folder';
  }

  @override
  String get emptyFolderTitle => 'مفيش ملاحظات هنا لسه';

  @override
  String get emptyFolderMessage =>
      'الملاحظات اللي تتحفظ في المجلد ده هتظهر هنا.';

  @override
  String noSearchResults({required String query}) {
    return 'مفيش ملاحظات فيها “$query”';
  }

  @override
  String get searchUnavailable => 'البحث مش متاح دلوقتي.';

  @override
  String get listErrorTitle => 'مقدرناش نفتح المجلد ده';

  @override
  String get listErrorMessage => 'حصلت مشكلة وإحنا بنقرأ الملاحظات على الجهاز.';

  @override
  String get notSyncedYet => 'لسه ما اتزامنتش';

  @override
  String get selectNoteTitle => 'اختار ملاحظة';

  @override
  String get selectNoteMessage =>
      'اختار ملاحظة من القائمة عشان تقرأها وتعدلها.';

  @override
  String get noteViewsLabel => 'عروض الملاحظة';

  @override
  String get tabNote => 'الملاحظة';

  @override
  String get tabLinks => 'الروابط';

  @override
  String get tabHistory => 'السجل';

  @override
  String get tabBacklinks => 'الروابط الواردة';

  @override
  String get tabGraph => 'الخريطة';

  @override
  String get contextLabel => 'سياق الملاحظة';

  @override
  String get contextTitle => 'السياق';

  @override
  String get showContext => 'إظهار لوحة السياق';

  @override
  String get hideContext => 'إخفاء لوحة السياق';

  @override
  String get openLocalMap => 'فتح الخريطة الذهنية';

  @override
  String get moreActions => 'إجراءات تانية';

  @override
  String get backToNotes => 'رجوع للملاحظات';

  @override
  String get propertiesTitle => 'الخصائص';

  @override
  String propertiesSummary({required int relations}) {
    String _temp0 = intl.Intl.pluralLogic(
      relations,
      locale: localeName,
      other: '$relations علاقة',
      many: '$relations علاقة',
      few: '$relations علاقات',
      two: 'علاقتين',
      one: 'علاقة واحدة',
      zero: 'مفيش علاقات',
    );
    return '$_temp0';
  }

  @override
  String get expandProperties => 'توسيع الخصائص';

  @override
  String get collapseProperties => 'طي الخصائص';

  @override
  String get relationsLabel => 'العلاقات';

  @override
  String get tagsLabel => 'الوسوم';

  @override
  String tagChip({required String tag}) {
    return '#$tag';
  }

  @override
  String get aiSuggestedBy => 'اقتراح من الذكاء الاصطناعي';

  @override
  String aiConfidence({required String value}) {
    return 'الثقة $value';
  }

  @override
  String get noReason => 'مفيش سبب متسجل.';

  @override
  String get relationReject => 'رفض';

  @override
  String get relationRetype => 'تغيير النوع';

  @override
  String get retypeTitle => 'تغيير نوع العلاقة';

  @override
  String get relationActionsHint => 'اضغط مطولًا للرفض أو تغيير النوع';

  @override
  String get relationUnresolved => 'مش موجودة في الخزنة لسه';

  @override
  String get backlinksByType => 'الروابط الواردة حسب النوع';

  @override
  String get noBacklinks => 'مفيش ملاحظات بتشاور هنا لسه.';

  @override
  String get backlinkKindLink => 'روابط في النص';

  @override
  String get localGraphTitle => 'الخريطة المحلية';

  @override
  String get openMap => 'فتح الخريطة';

  @override
  String get localGraphPlaceholder =>
      'الخريطة الذهنية للملاحظة دي بتتفتح في الخريطة.';

  @override
  String get historyTitle => 'السجل';

  @override
  String get historyAvailable => 'نسخ الملاحظة دي بتظهر هنا.';

  @override
  String get historyOffline => 'السجل محتاج اتصال. هيرجع أول ما تبقى أونلاين.';

  @override
  String get historyNotYetAvailable => 'السجل مش متاح لسه.';

  @override
  String get historyNotAllowed => 'السجل مش متاح للحساب ده.';

  @override
  String get deleteNote => 'مسح الملاحظة';

  @override
  String deleteNoteTitle({required String title}) {
    return 'تمسح “$title”؟';
  }

  @override
  String get deleteNoteMessage => 'الملاحظة هتتنقل لسلة المحذوفات.';

  @override
  String get shortcutsSearch => 'دور في الملاحظات';
}
