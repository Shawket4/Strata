// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'settings_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class SettingsLocalizationsAr extends SettingsLocalizations {
  SettingsLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get title => 'الإعدادات';

  @override
  String get groupYou => 'أنت';

  @override
  String get groupApp => 'التطبيق';

  @override
  String get groupAdmin => 'الإدارة';

  @override
  String get sectionAccount => 'الحساب';

  @override
  String get sectionDevices => 'الأجهزة';

  @override
  String get sectionReminders => 'التذكيرات';

  @override
  String get sectionAi => 'الذكاء الاصطناعي';

  @override
  String get sectionIntegrity => 'سلامة البيانات';

  @override
  String get sectionData => 'التصدير والاستيراد';

  @override
  String get sectionSync => 'المزامنة';

  @override
  String get sectionAdmin => 'المستخدمون';

  @override
  String get sectionAbout => 'عن التطبيق';

  @override
  String get settingsNav => 'أقسام الإعدادات';

  @override
  String get openAccount => 'الحساب وتسجيل الخروج';

  @override
  String atUsernameRole({required String username, required String role}) {
    return '@$username · $role';
  }

  @override
  String get roleAdmin => 'مسؤول';

  @override
  String get roleMember => 'عضو';

  @override
  String get displayName => 'الاسم الظاهر';

  @override
  String get username => 'اسم المستخدم';

  @override
  String get role => 'الدور';

  @override
  String get server => 'الخادم';

  @override
  String get language => 'اللغة';

  @override
  String get languageEn => 'English';

  @override
  String get languageAr => 'العربية';

  @override
  String get timezone => 'المنطقة الزمنية';

  @override
  String get changePasswordTitle => 'تغيير كلمة المرور';

  @override
  String get currentPassword => 'كلمة المرور الحالية';

  @override
  String get newPassword => 'كلمة المرور الجديدة';

  @override
  String get notAvailableYet => 'غير متاح بعد';

  @override
  String get signOut => 'تسجيل الخروج';

  @override
  String get thisDevice => 'الجهاز ده';

  @override
  String get deviceListNote =>
      'تغيير أسماء الأجهزة وإلغاؤها هييجي مع قائمة الأجهزة.';

  @override
  String get rename => 'إعادة تسمية';

  @override
  String get revoke => 'إلغاء';

  @override
  String get remindersOnDevice => 'التذكيرات على الجهاز ده';

  @override
  String get remindersOnDeviceHelp => 'كل جهاز التذكيرات شغالة عليه هينبهك.';

  @override
  String get remindersOffPermission => 'التذكيرات مقفولة على الجهاز ده';

  @override
  String get remindersOffPermissionBody =>
      'اسمح بالإشعارات (والمنبهات الدقيقة على أندرويد) لـ Strata من إعدادات النظام.';

  @override
  String get remindersPermissionUnknown => 'صلاحية الإشعارات لسه ماتفحصتش';

  @override
  String get remindersPermissionGranted => 'الإشعارات مسموحة';

  @override
  String get deliveryOs => 'بيوصلها النظام، حتى لو Strata مقفول';

  @override
  String get deliveryWhileRunning => 'بتوصل طول ما Strata شغال';

  @override
  String scheduledCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تذكير متجدول',
      many: '$count تذكيرًا متجدولًا',
      few: '$count تذكيرات متجدولة',
      two: 'تذكيران متجدولان',
      one: 'تذكير واحد متجدول',
      zero: 'مفيش تذكيرات متجدولة',
    );
    return '$_temp0';
  }

  @override
  String get defaultTime => 'ميعاد التذكير الافتراضي';

  @override
  String get defaultTimeHelp => 'للمهام اللي ليها تاريخ من غير ساعة';

  @override
  String get remindersSyncNote =>
      'لما تعلّم تذكير إنه اتعمل أو تأجله على جهاز، بيتشال من الأجهزة التانية مع المزامنة الجاية.';

  @override
  String get aiBody =>
      'حالة مزوّد الذكاء الاصطناعي، والحدود، والتصنيف التلقائي، والميزانية اليومية.';

  @override
  String get integrityBody =>
      'تحذيرات عن ملفات الخادم ماقدرش يقراها أو يفهرسها.';

  @override
  String get dataBody => 'نزّل الخزنة كلها كملف zip، أو استورد ملفات markdown.';

  @override
  String get exportVault => 'تصدير الخزنة (.zip)';

  @override
  String get importFiles => 'استيراد markdown…';

  @override
  String get available => 'متاح';

  @override
  String get unavailableOffline => 'محتاج اتصال';

  @override
  String get unavailableNotYet => 'غير متاح بعد';

  @override
  String get unavailableNotAllowed => 'غير متاح للحساب ده';

  @override
  String get offlineBody => 'ده محتاج الخادم. اتصل وحاول تاني.';

  @override
  String get notYetBody => 'الخادم لسه مش بيقدم ده.';

  @override
  String get notAllowedBody => 'الحساب ده مايقدرش يستخدم ده.';

  @override
  String get syncBody => 'حالة المزامنة، والتغييرات المنتظرة، والتعارضات.';

  @override
  String get openSyncStatus => 'افتح حالة المزامنة';

  @override
  String get adminBody =>
      'وافق على الحسابات الجديدة، وأوقف مستخدمين، وأعد تعيين كلمات المرور، وحدد مواعيد المسح.';

  @override
  String get openAdminUsers => 'افتح الإدارة ← المستخدمون';

  @override
  String get aboutBody =>
      'Strata بيحفظ ملاحظاتك كـ markdown على خادمك، ومعاها نسخة على كل جهاز.';

  @override
  String get licences => 'التراخيص';

  @override
  String get fontsNote =>
      'خطوط Cairo وIBM Plex Mono وQuicksand مضمّنة بترخيص SIL Open Font License.';

  @override
  String get back => 'رجوع';

  @override
  String get loading => 'جارٍ تحميل الإعدادات';

  @override
  String get loadFailed => 'تعذر تحميل الإعدادات';

  @override
  String errorGeneric({required String code}) {
    return 'حصل خطأ ($code).';
  }

  @override
  String get errorOffline => 'أنت غير متصل.';
}
