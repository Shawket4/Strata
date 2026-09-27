// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'directory_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class DirectoryLocalizationsAr extends DirectoryLocalizations {
  DirectoryLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get directoryTitle => 'الدليل';

  @override
  String get tabsLabel => 'أقسام الدليل';

  @override
  String get tabPeople => 'الأشخاص';

  @override
  String get tabCompanies => 'الشركات';

  @override
  String get tabDocuments => 'المستندات';

  @override
  String get tabPlaces => 'الأماكن';

  @override
  String tabWithCount({required String tab, required int count}) {
    return '$tab · $count';
  }

  @override
  String get searchPeople => 'ابحث في الأشخاص';

  @override
  String get searchCompanies => 'ابحث في الشركات';

  @override
  String get searchDocuments => 'دوّر على مستند أو اسأل هو فين';

  @override
  String get searchPlaces => 'ابحث في الأماكن';

  @override
  String get searchHint => 'ابحث بالعربي أو English';

  @override
  String get searchHintDocuments => 'فين…؟ / Where is…?';

  @override
  String get filtersLabel => 'الفلاتر';

  @override
  String get filterTag => 'وسم';

  @override
  String get filterRole => 'الدور';

  @override
  String get filterCompany => 'الشركة';

  @override
  String get filterIndustry => 'المجال';

  @override
  String get filterType => 'النوع';

  @override
  String get filterStatus => 'الحالة';

  @override
  String get filterPlace => 'المكان';

  @override
  String get filterExpiring => 'قرب ينتهي';

  @override
  String get filterHolder => 'الحائز';

  @override
  String get filtersUnavailable => 'الفلاتر مش متاحة لسه';

  @override
  String peopleCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count أشخاص',
      one: 'شخص واحد',
      zero: 'مفيش أشخاص',
    );
    return '$_temp0';
  }

  @override
  String companiesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count شركات',
      one: 'شركة واحدة',
      zero: 'مفيش شركات',
    );
    return '$_temp0';
  }

  @override
  String documentsCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count مستندات',
      one: 'مستند واحد',
      zero: 'مفيش مستندات',
    );
    return '$_temp0';
  }

  @override
  String placesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count أماكن',
      one: 'مكان واحد',
      zero: 'مفيش أماكن',
    );
    return '$_temp0';
  }

  @override
  String get emptyPeople => 'مفيش أشخاص لسه';

  @override
  String get emptyCompanies => 'مفيش شركات لسه';

  @override
  String get emptyDocuments => 'مفيش مستندات لسه';

  @override
  String get emptyPlaces => 'مفيش أماكن لسه';

  @override
  String get emptyMessage => 'بتتعمل من الإشارات في ملاحظاتك والتسجيلات.';

  @override
  String noMatches({required String query}) {
    return 'مفيش نتايج لـ “$query”';
  }

  @override
  String get noMatchesMessage =>
      'البحث بيدوّر في الأسماء والأسماء التانية بالعربي والإنجليزي.';

  @override
  String get suggestions => 'اقتراحات';

  @override
  String get suggestionsHint => 'مفيش حاجة بتتغير إلا لما توافق';

  @override
  String whoIs({required String mention}) {
    return 'مين “$mention”؟';
  }

  @override
  String whoIsCandidates({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count احتمالات',
      one: 'احتمال واحد',
      zero: 'مفيش شخص مطابق',
    );
    return '$_temp0';
  }

  @override
  String get possibleDuplicate => 'ممكن يكون مكرر';

  @override
  String get custodySuggestion => 'تحديث حيازة محتاج تأكيد';

  @override
  String get relationSuggestion => 'علاقة محتاجة مراجعة';

  @override
  String get taskSuggestion => 'مهمة من تسجيل';

  @override
  String get filingSuggestion => 'تصنيف محتاج مراجعة';

  @override
  String get otherSuggestion => 'اقتراح';

  @override
  String get accept => 'موافقة';

  @override
  String get dismiss => 'تجاهل';

  @override
  String get createPerson => 'إنشاء شخص…';

  @override
  String aiConfidence({required String value}) {
    return 'ذكاء اصطناعي · $value';
  }

  @override
  String get newPerson => 'شخص جديد';

  @override
  String get newCompany => 'شركة جديدة';

  @override
  String get nameField => 'الاسم';

  @override
  String get aliasesField => 'أسماء أخرى (كل اسم في سطر)';

  @override
  String get create => 'إنشاء';

  @override
  String get createAnyway => 'أنشئ برضه';

  @override
  String get cancel => 'إلغاء';

  @override
  String get alreadyExists => 'موجود بالفعل';

  @override
  String matchScore({required String level, required String score}) {
    return '$level · $score';
  }

  @override
  String get openExisting => 'افتح الموجود';

  @override
  String get colName => 'الاسم';

  @override
  String get colAliases => 'أسماء أخرى';

  @override
  String get colDetails => 'التفاصيل';

  @override
  String tableCaption({required String tab}) {
    return '$tab، حسب الاسم';
  }

  @override
  String get keyboardHint => 'تنقل · افتح الصفحة';

  @override
  String get preview => 'معاينة';

  @override
  String previewOf({required String title}) {
    return 'معاينة: $title';
  }

  @override
  String get closePreview => 'إغلاق المعاينة';

  @override
  String get openPage => 'افتح الصفحة';

  @override
  String get merge => 'دمج…';

  @override
  String get mergeUnavailable => 'الدمج مش متاح لسه';

  @override
  String get refreshInsights => 'تحديث الرؤى';

  @override
  String get refreshQueued => 'اتطلب تحديث الرؤى';

  @override
  String get summary => 'الملخص';

  @override
  String get aiMaintained => 'بيحدّثه الذكاء الاصطناعي';

  @override
  String get insights => 'رؤى';

  @override
  String get openItems => 'بنود مفتوحة';

  @override
  String get timeline => 'الخط الزمني';

  @override
  String get noSummary => 'مفيش ملخص لسه.';

  @override
  String get nothingYet => 'مفيش حاجة لسه.';

  @override
  String get relatedEntities => 'كيانات مرتبطة';

  @override
  String get noRelated => 'مفيش أشخاص أو شركات مرتبطة لسه.';

  @override
  String rejectRelation({required String type, required String title}) {
    return 'رفض علاقة $type $title';
  }

  @override
  String repointRelation({required String title}) {
    return 'تغيير $title';
  }

  @override
  String get repointUnavailable => 'تغيير الربط مش متاح لسه';

  @override
  String get mentioningNotes => 'ملاحظات بتذكره';

  @override
  String get newestFirst => 'الأحدث أولًا';

  @override
  String get noMentions => 'مفيش ملاحظات بتذكره لسه.';

  @override
  String get documentsSection => 'المستندات';

  @override
  String get entityGraph => 'رسم الكيان';

  @override
  String get yourNotes => 'ملاحظاتك';

  @override
  String get yourNotesHint => 'إنت بس اللي بتعدّل القسم ده · ## Notes';

  @override
  String get yourNotesUnavailable =>
      'تعديل ملاحظاتك من هنا مش متاح لسه. افتح الملاحظة وعدّل قسم ## Notes.';

  @override
  String get tabOverview => 'نظرة عامة';

  @override
  String tabNotes({required int count}) {
    return 'ملاحظات · $count';
  }

  @override
  String get tabGraph => 'الرسم';

  @override
  String get entityViews => 'عروض الكيان';

  @override
  String get kindPerson => 'شخص';

  @override
  String get kindCompany => 'شركة';

  @override
  String entitySubtitle({required String kind, required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ملاحظات بتذكره',
      one: 'ملاحظة واحدة بتذكره',
      zero: 'مفيش ملاحظات بتذكره',
    );
    return '$kind · $_temp0';
  }

  @override
  String get backToPeople => 'رجوع للأشخاص';

  @override
  String get backToCompanies => 'رجوع للشركات';

  @override
  String get openNote => 'افتح ملف الماركداون';

  @override
  String get pendingSync => 'لسه ما اتزامنش';

  @override
  String get loading => 'جارِ التحميل…';

  @override
  String get errorTitle => 'تعذّر التحميل';

  @override
  String errorMessage({required String code}) {
    return 'البيانات المحلية للتطبيق رجّعت خطأ ($code).';
  }

  @override
  String get notFoundTitle => 'الصفحة دي مش موجودة في الخزنة';

  @override
  String get notFoundMessage => 'ممكن تكون اتمسحت أو اتدمجت من جهاز تاني.';

  @override
  String get selectSomething => 'اختار عنصر علشان يظهر هنا';

  @override
  String dateShort({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String mentionDate({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }
}
