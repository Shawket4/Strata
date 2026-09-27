@Tags(['golden'])
library;

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_inbox/strata_inbox.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/fixtures.dart';
import '../helpers/harness.dart';

FakeCoreApi _fake(InboxView view) => FakeCoreApi()..inbox.add(view);

/// The inbox (captures with proposals, link-or-create) at compact / medium /
/// expanded × light/dark × LTR/RTL (1.0; compact also 2.0), and the custody,
/// duplicate and empty states on compact.
void main() {
  setUpAll(loadStrataFonts);

  for (final v in goldenMatrix()) {
    unawaited(
      goldenTest(
        'inbox $v',
        fileName: 'inbox_${v.id}',
        builder: () =>
            goldenScreen(v, const InboxScreen(), _fake(InboxFixtures.full)),
      ),
    );
  }
  for (final v in goldenMatrix(
    sizes: const {'compact': StrataTestSizes.compact},
  )) {
    unawaited(
      goldenTest(
        'inbox custody $v',
        fileName: 'inbox_custody_${v.id}',
        builder: () =>
            goldenScreen(v, const InboxScreen(), _fake(InboxFixtures.custody)),
      ),
    );
    unawaited(
      goldenTest(
        'inbox duplicate $v',
        fileName: 'inbox_duplicate_${v.id}',
        builder: () =>
            goldenScreen(v, const InboxScreen(), _fake(InboxFixtures.others)),
      ),
    );
    unawaited(
      goldenTest(
        'inbox empty $v',
        fileName: 'inbox_empty_${v.id}',
        builder: () =>
            goldenScreen(v, const InboxScreen(), _fake(InboxFixtures.empty)),
      ),
    );
  }
}
