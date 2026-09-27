import 'package:flutter/widgets.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';

/// Probe.
class A extends ConsumerWidget {
  /// Probe.
  const A({super.key});
  @override
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}

void main() {
  runApp(const A());
}
