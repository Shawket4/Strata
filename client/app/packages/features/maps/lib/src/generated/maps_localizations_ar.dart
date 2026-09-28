// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'maps_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class MapsLocalizationsAr extends MapsLocalizations {
  MapsLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get mapTitle => 'الخريطة';

  @override
  String get mapCompactTitle => 'الخريطة محتاجة نافذة أعرض';

  @override
  String get mapCompactMessage =>
      'افتح ملاحظة واستخدم خريطتها المحلية، أو كبّر النافذة.';

  @override
  String get mapEmptyTitle => 'مفيش حاجة على الخريطة لسه';

  @override
  String get mapEmptyMessage =>
      'الملاحظات والأشخاص والروابط بينهم هتظهر هنا وانت بتكتب.';

  @override
  String mapCounts({
    required int nodes,
    required int edges,
    required int clusters,
  }) {
    return '$nodes عقدة · $edges رابط · $clusters مجموعة';
  }

  @override
  String mapSemantics({required int nodes, required int edges}) {
    return 'الخريطة العامة فيها $nodes عقدة و$edges رابط';
  }

  @override
  String get lensLabel => 'العدسة';

  @override
  String get lensNotes => 'الملاحظات';

  @override
  String get lensPeople => 'الأشخاص';

  @override
  String get lensCompanies => 'الشركات';

  @override
  String get searchToFocus => 'ابحث للتركيز…';

  @override
  String get searchToFocusLabel => 'ابحث للتركيز';

  @override
  String get searchNoResults => 'مفيش ملاحظات مطابقة';

  @override
  String focusedOn({required String title}) {
    return 'التركيز على $title';
  }

  @override
  String get clearFocus => 'مسح';

  @override
  String get zoomGroup => 'التكبير';

  @override
  String get zoomIn => 'تكبير';

  @override
  String get zoomOut => 'تصغير';

  @override
  String get zoomToFit => 'ملاءمة العرض';

  @override
  String zoomLevelFar({required int percent}) {
    return '$percent٪ · العناوين: المجموعات';
  }

  @override
  String zoomLevelMid({required int percent}) {
    return '$percent٪ · العناوين: المحاور';
  }

  @override
  String zoomLevelNear({required int percent}) {
    return '$percent٪ · العناوين: الكل';
  }

  @override
  String get minimap => 'خريطة مصغّرة';

  @override
  String get filters => 'الفلاتر';

  @override
  String get mapFilters => 'فلاتر الخريطة';

  @override
  String get resetFilters => 'إعادة ضبط';

  @override
  String get closeFilters => 'إغلاق الفلاتر';

  @override
  String get edgeTypes => 'أنواع الروابط';

  @override
  String get edgeBodyLinks => 'روابط النص';

  @override
  String get nodeKinds => 'أنواع العقد';

  @override
  String get similarityTitle => 'تشابه بالذكاء الاصطناعي';

  @override
  String get similarityHelp =>
      'روابط باهتة بين الملاحظات المتشابهة. لا تُحفظ أبدًا في الخزنة.';

  @override
  String get clusterFocus => 'التركيز على مجموعة';

  @override
  String get allClusters => 'كل المجموعات';

  @override
  String clusterMembers({required int count}) {
    return '$count';
  }

  @override
  String hoverLinks({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count روابط',
      one: 'رابط واحد',
    );
    return '$_temp0';
  }

  @override
  String get openNote => 'افتح الملاحظة';

  @override
  String get openLocalMap => 'افتح الخريطة المحلية';

  @override
  String get localMap => 'خريطة محلية';

  @override
  String get depth => 'العمق';

  @override
  String depthValue({required int depth}) {
    return 'العمق $depth';
  }

  @override
  String recentreOn({required String title}) {
    return 'توسيط على $title';
  }

  @override
  String centreMapOn({required String title}) {
    return 'توسيط الخريطة على $title';
  }

  @override
  String get saveLayout => 'حفظ الترتيب';

  @override
  String get saveLayoutTarget => '← ‎.canvas';

  @override
  String get saveLayoutUnavailable => 'حفظ التخطيط يحتاج اتصالًا بالخادم';

  @override
  String get dragHint => 'اسحب ملاحظة فوق ملاحظة تانية لعمل علاقة';

  @override
  String get dragHintMore => '· اسحب لإعادة الترتيب، وبعدين احفظ الترتيب';

  @override
  String mindMapCounts({
    required int depth,
    required int nodes,
    required int edges,
  }) {
    return 'العمق $depth · $nodes عقدة · $edges رابط';
  }

  @override
  String mindMapSemantics({required String title}) {
    return 'الخريطة المحلية لـ $title';
  }

  @override
  String get mindMapNotFound => 'الملاحظة دي مش موجودة في الخزنة';

  @override
  String get mindMapNotFoundMessage =>
      'ممكن تكون اتمسحت أو اتنقلت من جهاز تاني.';

  @override
  String get selectedRelation => 'العلاقة المحددة';

  @override
  String edgeFromTo({required String from, required String to}) {
    return '$from ← $to';
  }

  @override
  String edgeSemantics({
    required String type,
    required String from,
    required String to,
  }) {
    return '$type: من $from إلى $to';
  }

  @override
  String get whyAi => 'ليه الذكاء الاصطناعي اقترح ده';

  @override
  String get reasonUnavailable =>
      'سبب الذكاء الاصطناعي للرابط ده مش ظاهر هنا لسه.';

  @override
  String get retype => 'تغيير النوع';

  @override
  String get reject => 'رفض';

  @override
  String get retypeTitle => 'تغيير نوع العلاقة';

  @override
  String get selectedNode => 'العقدة المحددة';

  @override
  String get clearSelection => 'إلغاء التحديد';

  @override
  String get relations => 'العلاقات';

  @override
  String get backlinks => 'الروابط الواردة';

  @override
  String get backToNote => 'رجوع للملاحظة';

  @override
  String get entityGraph => 'الرسم';

  @override
  String get openInMap => 'افتح في الخريطة';

  @override
  String miniGraphSemantics({required String title, required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count عناصر مرتبطة',
      one: 'عنصر واحد مرتبط',
    );
    return 'رسم $title: $_temp0';
  }

  @override
  String get loading => 'جارِ التحميل…';

  @override
  String get errorTitle => 'تعذّر التحميل';

  @override
  String errorMessage({required String code}) {
    return 'البيانات المحلية للتطبيق رجّعت خطأ ($code).';
  }

  @override
  String nodeSemantics({required String kind, required String title}) {
    return '$kind: $title';
  }

  @override
  String get mapBreadcrumb => 'الخريطة / خريطة محلية';

  @override
  String get similarityOffline => 'روابط التشابه تحتاج اتصالًا بالخادم.';

  @override
  String get showTags => 'الوسوم كعُقد';

  @override
  String kindWithCount({required String kind, required String count}) {
    return '$kind · $count';
  }

  @override
  String hoverLinksUpdated({required String links, required String updated}) {
    return '$links · $updated';
  }

  @override
  String get saveLayoutHint => 'يحفظ هذه الخريطة كملف JSON Canvas في خزنتك';

  @override
  String get layoutName => 'اسم الخريطة';

  @override
  String layoutSaved({required String path}) {
    return 'تم الحفظ في $path';
  }

  @override
  String get cancel => 'إلغاء';
}
