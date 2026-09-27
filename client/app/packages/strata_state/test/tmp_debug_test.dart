import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/testing.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_state/strata_state.dart';

class _P extends ConsumerWidget {
  const _P();
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final home = ref.watch(homeProvider);
    debugPrint('build $home');
    return const SizedBox();
  }
}
void main() {
  testWidgets('x', (tester) async {
    final fake = await pumpStrataScreen(tester, const _P());
    debugPrint('listener ${fake.home.hasListener}');
    fake.home.add(StrataFixtures.homeView);
    await tester.pump();
    debugPrint('one pump');
    debugPrint('after');
  });
}
