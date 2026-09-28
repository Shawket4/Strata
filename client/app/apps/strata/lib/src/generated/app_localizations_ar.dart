// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class AppLocalizationsAr extends AppLocalizations {
  AppLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get splashLoading => 'جارٍ فتح Strata';

  @override
  String get bootFailedTitle => 'Strata ماقدرش يبدأ';

  @override
  String bootFailedBody({required String code}) {
    return 'ماقدرناش نفتح قاعدة البيانات المحلية ($code).';
  }

  @override
  String get retry => 'أعد المحاولة';

  @override
  String get misconfiguredBuildTitle => 'النسخة دي مش هتقدر تبدأ';

  @override
  String misconfiguredBuildBody({required String reason}) {
    return 'النسخة دي من Strata اتبنت من غير عنوان خادم صالح ($reason). ثبّت نسخة مبنية بعنوان https:// في STRATA_SERVER_URL.';
  }

  @override
  String get notificationDone => 'تم';

  @override
  String get notificationSnooze => 'تأجيل';

  @override
  String get notificationOpen => 'فتح';

  @override
  String get channelName => 'التذكيرات';

  @override
  String get channelDescription => 'تذكيرات المهام من Strata';

  @override
  String folderSemantics({required String name, required int count}) {
    return '$name، $count ملاحظة';
  }

  @override
  String get accountLabel => 'الحساب';

  @override
  String get foldersTitle => 'المجلدات';

  @override
  String get pinnedNotesTitle => 'المثبّتة';

  @override
  String pinnedSemantics({required String title}) {
    return 'ملاحظة مثبّتة: $title';
  }
}
