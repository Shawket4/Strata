// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'documents_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class DocumentsLocalizationsAr extends DocumentsLocalizations {
  DocumentsLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get documents => 'المستندات';

  @override
  String get places => 'الأماكن';

  @override
  String get backToDocuments => 'رجوع للدليل';

  @override
  String get backToPlaces => 'رجوع للأماكن';

  @override
  String get moreActions => 'إجراءات أخرى';

  @override
  String get showContext => 'عرض السياق';

  @override
  String get contextLabel => 'السياق';

  @override
  String get breadcrumb => 'مسار التنقل';

  @override
  String get locationLabel => 'المكان';

  @override
  String get notFoundTitle => 'الصفحة دي مش موجودة في الخزنة';

  @override
  String get notFoundMessage => 'ممكن تكون اتمسحت أو اتدمجت من جهاز تاني.';

  @override
  String get loading => 'جارِ التحميل…';

  @override
  String get errorTitle => 'تعذّر التحميل';

  @override
  String errorMessage({required String code}) {
    return 'البيانات المحلية للتطبيق رجّعت خطأ ($code).';
  }

  @override
  String get pendingSync => 'لسه ما اتزامنش';

  @override
  String get documentKind => 'مستند';

  @override
  String get placeKind => 'مكان';

  @override
  String docSubtitle({required String type, required String copy}) {
    return '$type · $copy';
  }

  @override
  String expiresOn({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return 'ينتهي $dateString';
  }

  @override
  String get expiresLabel => 'ينتهي';

  @override
  String get renewalLabel => 'التجديد';

  @override
  String get reminderLabel => 'التذكير';

  @override
  String get typeLabel => 'النوع';

  @override
  String get concernsLabel => 'يخص';

  @override
  String get copiesLabel => 'النسخ';

  @override
  String copiesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count نسخ',
      one: 'نسخة واحدة',
      zero: 'مفيش نسخ',
    );
    return '$_temp0';
  }

  @override
  String get notAvailableYet => 'مش متاح لسه';

  @override
  String get whereItIs => 'مكانه فين';

  @override
  String get locationUnknown => 'المكان مش معروف';

  @override
  String withHolder({required String name}) {
    return 'مع $name';
  }

  @override
  String get nobodyHasIt => 'مش مع حد دلوقتي';

  @override
  String get lastWith => '· آخر واحد كان معاه';

  @override
  String get withLabel => 'مع';

  @override
  String custodyHeading({required DateTime date, required String kind}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString · $kind';
  }

  @override
  String get recordMove => 'تسجيل نقل';

  @override
  String get custodyHistory => 'سجل الحيازة';

  @override
  String get custodyHint => 'المكان والحائز بيتغيروا بس من خلال الأحداث دي';

  @override
  String custodyCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count أحداث · الأحدث أولًا',
      one: 'حدث واحد · الأحدث أولًا',
      zero: 'مفيش أحداث',
    );
    return '$_temp0';
  }

  @override
  String get noCustody => 'مفيش أحداث حيازة متسجلة لسه.';

  @override
  String get custodyStoredAt => 'اتحفظ في';

  @override
  String get custodyMovedTo => 'اتنقل إلى';

  @override
  String get custodyHandedTo => 'اتسلم لـ';

  @override
  String get custodyReturnedBy => 'رجّعه';

  @override
  String get custodySentTo => 'اتبعت إلى';

  @override
  String get custodyReceivedFrom => 'اتستلم من';

  @override
  String get custodyLost => 'ضاع';

  @override
  String get custodyFound => 'اتلاقى';

  @override
  String get custodyDestroyed => 'اتعدم';

  @override
  String get statusStored => 'محفوظ';

  @override
  String get statusCheckedOut => 'متسلّم';

  @override
  String get statusThirdParty => 'مع طرف تالت';

  @override
  String get statusLost => 'ضايع';

  @override
  String get statusDestroyed => 'متعدم';

  @override
  String get typeContract => 'عقد';

  @override
  String get typeId => 'بطاقة';

  @override
  String get typeLicence => 'رخصة';

  @override
  String get typeDeed => 'عقد ملكية';

  @override
  String get typeInvoice => 'فاتورة';

  @override
  String get typeCertificate => 'شهادة';

  @override
  String get typeOther => 'أخرى';

  @override
  String get copyOriginal => 'أصل';

  @override
  String get copyCertified => 'صورة معتمدة';

  @override
  String get copyCopy => 'صورة';

  @override
  String get copyDigital => 'رقمي';

  @override
  String get yourNotes => 'ملاحظاتك';

  @override
  String get yourNotesHint => 'إنت بس اللي بتعدّل ده';

  @override
  String get yourNotesUnavailable =>
      'تعديل ملاحظاتك من هنا مش متاح لسه. افتح الملاحظة وعدّل قسم ## Notes.';

  @override
  String get openNote => 'افتح الملاحظة';

  @override
  String get undoAi => 'تراجع';

  @override
  String get graph => 'الرسم';

  @override
  String get placesTree => 'الأماكن';

  @override
  String placesTreeLabel({required String place}) {
    return '$place والأماكن اللي جواه';
  }

  @override
  String get noSubPlaces => 'مفيش أماكن جواه';

  @override
  String get topLevelPlace => 'مستوى أعلى';

  @override
  String partOf({required String place}) {
    return 'جزء من $place';
  }

  @override
  String get everythingHere => 'كل اللي هنا';

  @override
  String everythingHereCaption({required String place}) {
    return 'المستندات في $place، ومعاها الأماكن اللي جواه';
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
  String get noDocumentsHere => 'مفيش حاجة متخزنة هنا لسه.';

  @override
  String get colDocument => 'المستند';

  @override
  String get colWhere => 'فين';

  @override
  String get colHolder => 'الحائز';

  @override
  String get colStatus => 'الحالة';

  @override
  String get recentMovements => 'آخر التحركات';

  @override
  String get noMovements => 'مفيش تحركات متسجلة هنا لسه.';

  @override
  String get aliases => 'أسماء أخرى';

  @override
  String recordMoveSubtitle({required String document, required String place}) {
    return '$document · دلوقتي في $place';
  }

  @override
  String recordMoveSubtitleUnknown({required String document}) {
    return '$document';
  }

  @override
  String get whatHappened => 'إيه اللي حصل';

  @override
  String get eventMovedTo => 'اتنقل إلى';

  @override
  String get eventHandedTo => 'اتسلم لـ';

  @override
  String get eventReturned => 'رجع';

  @override
  String get eventSentTo => 'اتبعت لطرف تالت';

  @override
  String get eventLost => 'ضاع';

  @override
  String get toPlace => 'إلى مكان';

  @override
  String get holderAfter => 'الحائز بعدها';

  @override
  String get nobody => 'مفيش حد';

  @override
  String get documentField => 'المستند';

  @override
  String get dateField => 'التاريخ';

  @override
  String get dateToday => 'النهارده';

  @override
  String get pickDate => 'اختار تاريخ';

  @override
  String get noteField => 'ملاحظة (اختياري)';

  @override
  String get recordMoveFooter => 'بيضيف حدث موثّق لـ ## Custody';

  @override
  String get recordMoveSubmit => 'سجّل النقل';

  @override
  String get recordMoveUnavailable => 'تسجيل النقل من التطبيق مش متاح لسه.';

  @override
  String get cancel => 'إلغاء';

  @override
  String get close => 'إغلاق';

  @override
  String get searchPlaces => 'ابحث في الأماكن';

  @override
  String get searchPeople => 'ابحث في الأشخاص';

  @override
  String get mentionsUnavailable => 'الملاحظات اللي بتذكره هتظهر هنا.';

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
  String get userNotesEmpty => 'لم تُكتب ملاحظات بعد.';

  @override
  String get userNotesEdit => 'تعديل الملاحظات';

  @override
  String get userNotesField => 'ملاحظاتك';

  @override
  String get userNotesSave => 'حفظ الملاحظات';

  @override
  String get userNotesSaved => 'تم حفظ الملاحظات';

  @override
  String userNotesFailed({required String code}) {
    return 'تعذّر حفظ ملاحظاتك ($code).';
  }
}
