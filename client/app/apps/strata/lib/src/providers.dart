import 'package:riverpod_annotation/riverpod_annotation.dart';
import 'package:strata_ui/strata_ui.dart';

part 'providers.g.dart';

/// Sync status streamed by the Rust client core.
///
/// Providers only adapt core streams into rebuilds (PLAN §11.1, L15). Until
/// `strata_bridge` exists there is no source, so the stream is empty and no
/// sync pill is shown; tests override it with fixtures.
@Riverpod(keepAlive: true)
Stream<SyncStatus> syncStatus(Ref ref) => const Stream.empty();
