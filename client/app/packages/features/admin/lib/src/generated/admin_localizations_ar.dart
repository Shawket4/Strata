// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'admin_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class AdminLocalizationsAr extends AdminLocalizations {
  AdminLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get usersTitle => 'المستخدمون';

  @override
  String get breadcrumb => 'الإعدادات / الإدارة';

  @override
  String get intro =>
      'المسؤولين بيديروا الحسابات. محدش غير صاحب الحساب يقدر يشوف ملاحظاته.';

  @override
  String get createAccount => 'إنشاء حساب';

  @override
  String get pendingTitle => 'في انتظار الموافقة';

  @override
  String get pendingEmpty => 'مفيش حسابات في انتظار الموافقة.';

  @override
  String get allUsers => 'كل المستخدمين';

  @override
  String requested({required String username, required String date}) {
    return '@$username · طلب $date';
  }

  @override
  String userSubtitle({required String username, required String role}) {
    return '@$username · $role';
  }

  @override
  String get approve => 'موافقة';

  @override
  String get reject => 'رفض';

  @override
  String approveSemantics({required String name}) {
    return 'الموافقة على $name';
  }

  @override
  String rejectSemantics({required String name}) {
    return 'رفض $name';
  }

  @override
  String get roleAdmin => 'مسؤول';

  @override
  String get roleMember => 'عضو';

  @override
  String get statusActive => 'نشط';

  @override
  String get statusDisabled => 'موقوف';

  @override
  String get statusPending => 'قيد الانتظار';

  @override
  String get statusRejected => 'مرفوض';

  @override
  String statusDeletion({required String date}) {
    return 'المسح $date';
  }

  @override
  String get statusDeletionNoDate => 'المسح متحدد';

  @override
  String statusOther({required String status}) {
    return '$status';
  }

  @override
  String get exportNotDownloaded => 'التصدير لسه ماتنزلش';

  @override
  String get columnName => 'الاسم';

  @override
  String get columnUsername => 'اسم المستخدم';

  @override
  String get columnRole => 'الدور';

  @override
  String get columnStatus => 'الحالة';

  @override
  String get columnCreated => 'تاريخ الإنشاء';

  @override
  String get columnActions => 'الإجراءات';

  @override
  String get disable => 'إيقاف';

  @override
  String get enable => 'تفعيل';

  @override
  String get resetPassword => 'إعادة تعيين كلمة المرور';

  @override
  String get scheduleDeletion => 'تحديد ميعاد المسح';

  @override
  String get cancelDeletion => 'إلغاء المسح';

  @override
  String disableSemantics({required String name}) {
    return 'إيقاف $name';
  }

  @override
  String enableSemantics({required String name}) {
    return 'تفعيل $name';
  }

  @override
  String resetSemantics({required String name}) {
    return 'إعادة تعيين كلمة مرور $name';
  }

  @override
  String deleteSemantics({required String name}) {
    return 'مسح $name…';
  }

  @override
  String cancelDeletionSemantics({required String name}) {
    return 'إلغاء مسح $name';
  }

  @override
  String userActions({required String name}) {
    return 'إجراءات $name';
  }

  @override
  String get footnoteDisable =>
      'الإيقاف بيسجل خروج المستخدم من كل مكان وبيمسح بياناته من أجهزته.';

  @override
  String get footnoteReset =>
      'إعادة التعيين بتديك كلمة مرور لمرة واحدة توصلها له.';

  @override
  String scheduleTitle({required String username}) {
    return 'تحديد ميعاد مسح @$username؟';
  }

  @override
  String scheduleBody({required String name}) {
    return 'حساب $name هيتقفل من كل الأجهزة ويتمسح لما فترة السماح تخلص. لحد ساعتها $name يقدر يدخل بس عشان ينزّل نسخة من ملاحظاته. انت مش هتشوف النسخة دي.';
  }

  @override
  String scheduleBodyDated({required String name, required String date}) {
    return 'حساب $name هيتقفل من كل الأجهزة. $date. لحد ساعتها $name يقدر يدخل بس عشان ينزّل نسخة من ملاحظاته. انت مش هتشوف النسخة دي.';
  }

  @override
  String get cancel => 'إلغاء';

  @override
  String oneTimeTitle({required String username}) {
    return 'كلمة مرور لمرة واحدة لـ @$username';
  }

  @override
  String oneTimeBody({required String name}) {
    return 'وصّلها بشكل خاص. بتظهر مرة واحدة بس؛ و$name لازم يختار كلمة مرور جديدة بعد تسجيل الدخول.';
  }

  @override
  String get copy => 'نسخ';

  @override
  String get copied => 'اتنسخت';

  @override
  String get done => 'تم';

  @override
  String get offlineTitle => 'الإدارة ← المستخدمون محتاجة اتصال';

  @override
  String get offlineBody => 'إدارة الحسابات بتتم على الخادم. اتصل وحاول تاني.';

  @override
  String get notYetTitle => 'غير متاح بعد';

  @override
  String get notYetBody => 'الخادم ده لسه مش بيقدم إدارة الحسابات.';

  @override
  String get notAllowedTitle => 'للمسؤولين فقط';

  @override
  String get notAllowedBody => 'المسؤولين بس يقدروا يديروا المستخدمين.';

  @override
  String get loadFailed => 'تعذر تحميل المستخدمين';

  @override
  String get retry => 'أعد المحاولة';

  @override
  String get loading => 'جارٍ تحميل المستخدمين';

  @override
  String errorGeneric({required String code}) {
    return 'حصل خطأ ($code).';
  }

  @override
  String get errorOffline => 'أنت غير متصل.';

  @override
  String atUsername({required String username}) {
    return '@$username';
  }

  @override
  String get you => 'انت';

  @override
  String get yourAccount => 'حسابك';

  @override
  String get searchLabel => 'ابحث في المستخدمين';

  @override
  String noMatches({required String query}) {
    return 'مفيش مستخدم مطابق لـ“$query”.';
  }

  @override
  String roleSemantics({required String name}) {
    return 'دور $name';
  }

  @override
  String get makeAdmin => 'خليه مسؤول';

  @override
  String get makeMember => 'خليه عضو';

  @override
  String get passwordChangePending => 'لازم يختار كلمة سر جديدة';

  @override
  String get createTitle => 'إنشاء حساب';

  @override
  String get createBody =>
      'الحساب بيبقى نشط على طول. ابعت كلمة السر بشكل خاص؛ هو هيختار واحدة جديدة بعد ما يدخل.';

  @override
  String get fieldUsername => 'اسم المستخدم';

  @override
  String get fieldDisplayName => 'الاسم الظاهر';

  @override
  String get fieldPassword => 'كلمة سر مؤقتة';

  @override
  String get fieldRole => 'الدور';

  @override
  String get create => 'إنشاء';

  @override
  String created({required String username}) {
    return 'اتعمل حساب @$username';
  }

  @override
  String approved({required String name}) {
    return 'تمت الموافقة على $name';
  }

  @override
  String rejected({required String name}) {
    return 'اترفض $name';
  }
}
