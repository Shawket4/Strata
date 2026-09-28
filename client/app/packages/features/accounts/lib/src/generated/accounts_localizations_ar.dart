// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'accounts_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class AccountsLocalizationsAr extends AccountsLocalizations {
  AccountsLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get signInTitle => 'تسجيل الدخول';

  @override
  String get signInSubtitle =>
      'ملاحظاتك بتفضل على الخادم بتاعك وعلى الجهاز ده.';

  @override
  String get signInSubtitleWide => 'حساب واحد لكل جهاز. ملاحظاتك خاصة بيك.';

  @override
  String get brandKicker => 'معرفة على خادمك';

  @override
  String get brandTagline =>
      'الملاحظات السريعة بتتصنف، والملاحظات بتترابط، والناس والشركات بيفضلوا محدثين. بالعربي والإنجليزي، أونلاين أو أوفلاين.';

  @override
  String get fieldServer => 'الخادم';

  @override
  String get fieldUsername => 'اسم المستخدم';

  @override
  String get fieldPassword => 'كلمة المرور';

  @override
  String get fieldConfirmPassword => 'تأكيد كلمة المرور';

  @override
  String get fieldNewPassword => 'كلمة المرور الجديدة';

  @override
  String get fieldCurrentPassword => 'كلمة المرور الحالية';

  @override
  String get fieldDeviceName => 'اسم الجهاز';

  @override
  String get fieldDisplayName => 'الاسم الظاهر';

  @override
  String get deviceNameHint =>
      'بيظهر في قائمة أجهزتك المسجلة. تقدر تغيّره بعدين.';

  @override
  String get showPassword => 'إظهار كلمة المرور';

  @override
  String get hidePassword => 'إخفاء كلمة المرور';

  @override
  String get enterToSignIn => 'لتسجيل الدخول';

  @override
  String get newHere => 'جديد هنا؟';

  @override
  String get createAccount => 'إنشاء حساب';

  @override
  String get forgotPassword => 'نسيتها؟ اطلب من المسؤول يعيد تعيينها.';

  @override
  String get knownAccountsTitle => 'الحسابات على الجهاز ده';

  @override
  String continueAs({required String name}) {
    return 'كمّل باسم $name';
  }

  @override
  String signUpOn({required String server}) {
    return 'على $server';
  }

  @override
  String get approvalNotice =>
      'لازم مسؤول يوافق على الحسابات الجديدة. هتقدر تسجل دخول بعد الموافقة.';

  @override
  String get requestAccount => 'اطلب حساب';

  @override
  String get haveAccount => 'عندك حساب بالفعل؟';

  @override
  String get backToSignIn => 'رجوع لتسجيل الدخول';

  @override
  String get passwordsMatch => 'كلمتا المرور متطابقتان';

  @override
  String get passwordsDiffer => 'كلمتا المرور مش متطابقتين';

  @override
  String get pendingPill => 'قيد الانتظار';

  @override
  String get pendingTitle => 'في انتظار الموافقة';

  @override
  String pendingBody({required String username, required String requested}) {
    return 'طلبك لـ @$username اتبعت $requested.';
  }

  @override
  String get pendingNext => 'هتقدر تسجل دخول أول ما مسؤول يوافق على حسابك.';

  @override
  String get checkAgain => 'اتأكد تاني';

  @override
  String get useAnotherAccount => 'استخدم حساب تاني';

  @override
  String get rejectedTitle => 'طلب الحساب ده ماتقبلش.';

  @override
  String get rejectedBody =>
      'كلم المسؤول لو ده غلط. مفيش حاجة اتخزنت على الجهاز ده.';

  @override
  String get roleAdmin => 'مسؤول';

  @override
  String get roleMember => 'عضو';

  @override
  String get accountSheetLabel => 'الحساب';

  @override
  String atUsername({required String username}) {
    return '@$username';
  }

  @override
  String get thisDevice => 'الجهاز ده';

  @override
  String get devices => 'الأجهزة';

  @override
  String get adminUsers => 'الإدارة ← المستخدمون';

  @override
  String get signOut => 'تسجيل الخروج';

  @override
  String unsyncedTitle({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تغيير لسه ماتزامنش',
      many: '$count تغييرًا لسه ماتزامنش',
      few: '$count تغييرات لسه ماتزامنتش',
      two: 'تغييران لسه ماتزامنوش',
      one: 'تغيير واحد لسه ماتزامنش',
    );
    return '$_temp0';
  }

  @override
  String get unsyncedBody =>
      'تسجيل الخروج بيمسح بيانات الحساب ده من الجهاز. زامن الأول علشان تحتفظ بيها.';

  @override
  String get syncNow => 'زامن الآن';

  @override
  String get signOutAnyway => 'سجل خروج على أي حال';

  @override
  String get cancel => 'إلغاء';

  @override
  String get disabledTitle => 'الحساب ده اتوقف';

  @override
  String disabledBody({required String username}) {
    return 'مسؤول أوقف @$username. البيانات المحلية على الجهاز ده هتتمسح.';
  }

  @override
  String get disabledServerNote =>
      'الملاحظات اللي اتزامنت بتفضل مع الحساب على الخادم.';

  @override
  String unsyncedCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تغيير غير متزامن',
      many: '$count تغييرًا غير متزامن',
      few: '$count تغييرات غير متزامنة',
      two: 'تغييران غير متزامنين',
      one: 'تغيير واحد غير متزامن',
      zero: 'مفيش تغييرات غير متزامنة',
    );
    return '$_temp0';
  }

  @override
  String get onlyOnDevice => 'على الجهاز ده بس';

  @override
  String get exportFirst => 'صدّرها الأول';

  @override
  String get exportFirstNote =>
      'التصدير بيحفظها كملفات markdown تقدر تحتفظ بيها.';

  @override
  String get removeAndSignOut => 'امسح وسجل خروج';

  @override
  String get deletionTitle => 'حسابك بيتمسح';

  @override
  String daysLeft({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count يوم متبقي',
      many: '$count يومًا متبقيًا',
      few: '$count أيام متبقية',
      two: 'يومان متبقيان',
      one: 'يوم واحد متبقي',
      zero: 'هيتمسح النهارده',
    );
    return '$_temp0';
  }

  @override
  String deletionBody({required String username, required String date}) {
    return 'مسؤول حدد مسح @$username يوم $date. نزّل ملاحظاتك قبلها.';
  }

  @override
  String get downloadExport => 'تنزيل التصدير (.zip)';

  @override
  String unsyncedNotInExport({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تغيير على الجهاز ده ماتزامنش',
      many: '$count تغييرًا على الجهاز ده ماتزامنش',
      few: '$count تغييرات على الجهاز ده ماتزامنتش',
      two: 'تغييران على الجهاز ده ماتزامنوش',
      one: 'تغيير واحد على الجهاز ده ماتزامنش',
    );
    return '$_temp0';
  }

  @override
  String get notInExport => 'مش موجودة في التصدير.';

  @override
  String get saveAsFile => 'احفظها في ملف';

  @override
  String get deleteNow => 'امسح دلوقتي';

  @override
  String get readOnlyNote => 'ملاحظاتك للقراءة بس لحد وقتها.';

  @override
  String get passwordChangeTitle => 'اختار كلمة مرور جديدة';

  @override
  String get passwordChangeBody =>
      'مسؤول أعاد تعيين كلمة المرور. اختار واحدة جديدة علشان تكمل.';

  @override
  String get changePassword => 'تغيير كلمة المرور';

  @override
  String get errorInvalidCredentials => 'اسم المستخدم أو كلمة المرور غلط';

  @override
  String get errorOffline => 'مش قادر أوصل للخادم. اتأكد من العنوان والاتصال.';

  @override
  String get errorAccountPending => 'الحساب ده في انتظار الموافقة.';

  @override
  String get errorAccountRejected => 'طلب الحساب ده ماتقبلش.';

  @override
  String get errorAccountDisabled => 'الحساب ده اتوقف من مسؤول.';

  @override
  String get errorAccountDeletion => 'الحساب ده متحدد له ميعاد مسح.';

  @override
  String get errorSessionExpired => 'الجلسة انتهت. سجل دخول تاني.';

  @override
  String get errorRateLimited => 'محاولات كتير. استنى دقيقة وحاول تاني.';

  @override
  String errorInvalidInput({required String field}) {
    return 'راجع خانة $field.';
  }

  @override
  String errorServer({required String status}) {
    return 'الخادم رد بخطأ ($status).';
  }

  @override
  String get errorNotAvailable => 'ده مش متاح لسه.';

  @override
  String errorGeneric({required String code}) {
    return 'حصل خطأ ($code).';
  }

  @override
  String unsyncedItem({required String kind, required String time}) {
    return '$kind · $time';
  }

  @override
  String get strengthWeak => 'ضعيفة';

  @override
  String get strengthFair => 'مقبولة';

  @override
  String get strengthStrong => 'قوية';

  @override
  String strengthDetail({required int length, required int min}) {
    String _temp0 = intl.Intl.pluralLogic(
      length,
      locale: localeName,
      other: '$length حرف',
      few: '$length حروف',
      two: 'حرفين',
      one: 'حرف واحد',
    );
    return '· $_temp0 · على الأقل $min';
  }

  @override
  String thisDeviceDetail({required String name, required String signedIn}) {
    return '$name · دخل $signedIn';
  }

  @override
  String pendingCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count مستني',
      few: '$count مستنيين',
      two: 'اتنين مستنيين',
      one: 'واحد مستني',
      zero: 'مفيش حد مستني',
    );
    return '$_temp0';
  }

  @override
  String exportSaved({required String label}) {
    return 'اتحفظ $label';
  }

  @override
  String unsyncedSaved({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'اتحفظ $count تغيير',
      few: 'اتحفظ $count تغييرات',
      two: 'اتحفظ تغييرين',
      one: 'اتحفظ تغيير واحد',
    );
    return '$_temp0';
  }

  @override
  String deleteNowTitle({required String username}) {
    return 'تمسح @$username دلوقتي؟';
  }

  @override
  String get deleteNowBody =>
      'الحساب وكل الملاحظات على السيرفر هيتمسحوا دلوقتي. مفيش رجوع.';

  @override
  String get deleteAnyway => 'امسح برضه';

  @override
  String get fieldTemporaryPassword => 'كلمة السر المؤقتة';

  @override
  String get passwordChanged => 'اتغيرت كلمة السر';

  @override
  String get stillPending => 'لسه مستني الموافقة.';

  @override
  String get strengthTooShort => 'قصيرة';
}
