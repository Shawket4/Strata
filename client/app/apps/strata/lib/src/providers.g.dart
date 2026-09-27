// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'providers.dart';

// **************************************************************************
// RiverpodGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint, type=warning
/// Sync status streamed by the Rust client core.
///
/// Providers only adapt core streams into rebuilds (PLAN §11.1, L15). Until
/// `strata_bridge` exists there is no source, so the stream is empty and no
/// sync pill is shown; tests override it with fixtures.

@ProviderFor(syncStatus)
final syncStatusProvider = SyncStatusProvider._();

/// Sync status streamed by the Rust client core.
///
/// Providers only adapt core streams into rebuilds (PLAN §11.1, L15). Until
/// `strata_bridge` exists there is no source, so the stream is empty and no
/// sync pill is shown; tests override it with fixtures.

final class SyncStatusProvider
    extends
        $FunctionalProvider<
          AsyncValue<SyncStatus>,
          SyncStatus,
          Stream<SyncStatus>
        >
    with $FutureModifier<SyncStatus>, $StreamProvider<SyncStatus> {
  /// Sync status streamed by the Rust client core.
  ///
  /// Providers only adapt core streams into rebuilds (PLAN §11.1, L15). Until
  /// `strata_bridge` exists there is no source, so the stream is empty and no
  /// sync pill is shown; tests override it with fixtures.
  SyncStatusProvider._()
    : super(
        from: null,
        argument: null,
        retry: null,
        name: r'syncStatusProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$syncStatusHash();

  @$internal
  @override
  $StreamProviderElement<SyncStatus> $createElement($ProviderPointer pointer) =>
      $StreamProviderElement(pointer);

  @override
  Stream<SyncStatus> create(Ref ref) {
    return syncStatus(ref);
  }
}

String _$syncStatusHash() => r'41e8c45cd38c709428dc3915520ff2371c7bcd14';
