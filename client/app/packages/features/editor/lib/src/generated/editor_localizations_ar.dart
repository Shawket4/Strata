// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'editor_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class EditorLocalizationsAr extends EditorLocalizations {
  EditorLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get editorScreenTitle => 'محرر الملاحظات';

  @override
  String get editorBodyLabel => 'نص الملاحظة';

  @override
  String get statusUnsaved => 'تعديلات غير محفوظة';

  @override
  String get conflictBannerTitle => 'الملاحظة دي اتعدلت كمان على السيرفر';

  @override
  String get conflictBannerMessage =>
      'تعديلك متداخل مع نسخة السيرفر. حل التعارض عشان المزامنة تكمل للملاحظة دي.';

  @override
  String get conflictResolve => 'حل التعارض';

  @override
  String get noteNotFoundTitle => 'الملاحظة مش موجودة';

  @override
  String get noteNotFoundMessage => 'ممكن تكون اتمسحت أو اتنقلت.';

  @override
  String get noteLoadErrorTitle => 'مقدرناش نفتح الملاحظة دي';

  @override
  String get noteLoadErrorMessage =>
      'حصلت مشكلة وإحنا بنقرأ الملاحظة دي على الجهاز.';

  @override
  String get saveFailed => 'مقدرناش نحفظ. النص لسه موجود.';

  @override
  String get toolbarLabel => 'التنسيق';

  @override
  String get toolUndo => 'تراجع';

  @override
  String get toolRedo => 'إعادة';

  @override
  String get toolHeading => 'عنوان';

  @override
  String get toolBold => 'عريض';

  @override
  String get toolItalic => 'مائل';

  @override
  String get toolBulletList => 'قائمة نقطية';

  @override
  String get toolChecklist => 'مهمة';

  @override
  String get toolWikilink => 'إدراج رابط لملاحظة';

  @override
  String get toolMention => 'إشارة لشخص أو شركة';

  @override
  String get toolTag => 'وسم';

  @override
  String get toolHideKeyboard => 'إخفاء الكيبورد';

  @override
  String get toolOpenLink => 'فتح الرابط';

  @override
  String taskComplete({required String task}) {
    return 'إنهاء المهمة: $task';
  }

  @override
  String taskReopen({required String task}) {
    return 'إعادة فتح المهمة: $task';
  }

  @override
  String get taskNotSynced => 'سطر مهمة (احفظ الأول)';

  @override
  String get suggestionsLabel => 'اقتراحات';

  @override
  String get suggestionsNone => 'مفيش نتائج';

  @override
  String get suggestionsDismiss => 'إخفاء الاقتراحات';

  @override
  String get statusBarMarkdown => 'Markdown';

  @override
  String get statusBarRtl => 'اتجاه تلقائي لكل فقرة';

  @override
  String get shortcutSave => 'حفظ';

  @override
  String get suggestionsKindWikiLink => 'ملاحظات';

  @override
  String get suggestionsKindMention => 'أشخاص وشركات';

  @override
  String get suggestionsKindTag => 'وسوم';

  @override
  String get suggestionsKindBlock => 'فقرات';

  @override
  String get showMarkdown => 'اعرض الماركداون';

  @override
  String get hideMarkdown => 'اخفي الماركداون';
}
