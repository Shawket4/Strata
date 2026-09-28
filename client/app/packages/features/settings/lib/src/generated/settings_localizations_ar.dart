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
  String get signOut => 'تسجيل الخروج';

  @override
  String get thisDevice => 'الجهاز ده';

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

  @override
  String get passwordGroup => 'كلمة المرور';

  @override
  String get edit => 'تعديل';

  @override
  String get editDisplayNameTitle => 'الاسم الظاهر';

  @override
  String get editTimezoneTitle => 'المنطقة الزمنية';

  @override
  String get timezoneHelp =>
      'اسم IANA زي Africa/Cairo. التواريخ والتذكيرات بتمشي عليه.';

  @override
  String get save => 'حفظ';

  @override
  String get cancel => 'إلغاء';

  @override
  String get passwordChanged => 'اتغيرت كلمة السر';

  @override
  String get saved => 'اتحفظ';

  @override
  String get refresh => 'تحديث';

  @override
  String get thisDeviceBadge => 'الجهاز ده';

  @override
  String deviceDetail({required String lastSeen, required String signedIn}) {
    return 'آخر ظهور $lastSeen · دخل $signedIn';
  }

  @override
  String deviceReminders({required String name}) {
    return 'التذكيرات على $name';
  }

  @override
  String renameDevice({required String name}) {
    return 'تغيير اسم $name';
  }

  @override
  String revokeDevice({required String name}) {
    return 'تسجيل خروج $name';
  }

  @override
  String get renameTitle => 'تغيير اسم الجهاز';

  @override
  String revokeTitle({required String name}) {
    return 'تسجيل خروج $name؟';
  }

  @override
  String get revokeBody =>
      'الجهاز هيتسجل خروجه والنسخة المحلية من ملاحظاتك هتتمسح أول ما يتصل.';

  @override
  String get noDevices => 'مفيش أجهزة لسه.';

  @override
  String get snooze => 'مدة التأجيل';

  @override
  String snoozeMinutes({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count دقيقة',
      few: '$count دقايق',
      two: 'دقيقتين',
      one: 'دقيقة',
    );
    return '$_temp0';
  }

  @override
  String get quietHours => 'ساعات الهدوء';

  @override
  String get quietHoursHelp =>
      'مفيش تذكير بيرن في الوقت ده؛ بيستنى لحد ما يخلص.';

  @override
  String get quietFrom => 'من (HH:MM)';

  @override
  String get quietUntil => 'لحد (HH:MM)';

  @override
  String get timeField => 'الوقت (HH:MM)';

  @override
  String aiOn({required String provider}) {
    return 'الذكاء الاصطناعي شغال · $provider';
  }

  @override
  String get aiOnNoProvider => 'الذكاء الاصطناعي شغال';

  @override
  String get aiOff => 'الذكاء الاصطناعي مقفول على السيرفر ده';

  @override
  String aiQueue({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count مهمة مستنية',
      few: '$count مهام مستنية',
      two: 'مهمتين مستنيين',
      one: 'مهمة واحدة مستنية',
      zero: 'مفيش حاجة مستنية',
    );
    return '$_temp0';
  }

  @override
  String get aiBudget => 'الميزانية اليومية';

  @override
  String aiEmbeddings({required int percent}) {
    return 'فهرس البحث $percent%';
  }

  @override
  String get aiThresholdsNote =>
      'الحدود والتصنيف التلقائي بيتظبطوا على السيرفر.';

  @override
  String get integrityEmpty => 'مفيش تحذيرات.';

  @override
  String get integrityTempFile => 'اتمسح ملف مؤقت متساب';

  @override
  String get integrityUncommitted => 'اتلاقت تغييرات مش متسجلة واتحفظت';

  @override
  String get integrityIdAssigned => 'ملاحظة من غير ID خدت واحد';

  @override
  String get integrityOutOfBand => 'ملف اتعدل برا Strata واتقرا تاني';

  @override
  String get integrityMissingFile => 'فيه ملف ناقص من الخزنة';

  @override
  String get integritySidecar => 'ملف بيانات اتصلح';

  @override
  String get integrityOrphanSidecar => 'اتمسح ملف بيانات ملوش ملاحظة';

  @override
  String get integrityIndexRepaired => 'فهرس البحث اتصلح';

  @override
  String get integrityRolledBack => 'كتابة اتقطعت ورجعت';

  @override
  String get integrityRecovered => 'نتيجة تغيير اترجعت';

  @override
  String integrityOther({required String kind}) {
    return 'تحذير: $kind';
  }

  @override
  String get exportTitle => 'تصدير الخزنة';

  @override
  String get importTitle => 'استيراد ماركداون';

  @override
  String get exportPath => 'احفظ في (مسار الملف)';

  @override
  String get importPath => 'ملف zip (المسار)';

  @override
  String exportDone({required String label}) {
    return 'اتصدّر $label';
  }

  @override
  String importDone({required int imported, required int skipped}) {
    return 'اتستورد $imported · اتساب $skipped';
  }

  @override
  String get exportAction => 'تصدير';

  @override
  String get importAction => 'استيراد';
}
