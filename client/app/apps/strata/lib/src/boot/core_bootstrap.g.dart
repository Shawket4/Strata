// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'core_bootstrap.dart';

// **************************************************************************
// RiverpodGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint, type=warning
/// The bootstrap in use (overridden in tests).

@ProviderFor(coreBootstrap)
final coreBootstrapProvider = CoreBootstrapProvider._();

/// The bootstrap in use (overridden in tests).

final class CoreBootstrapProvider
    extends $FunctionalProvider<CoreBootstrap, CoreBootstrap, CoreBootstrap>
    with $Provider<CoreBootstrap> {
  /// The bootstrap in use (overridden in tests).
  CoreBootstrapProvider._()
    : super(
        from: null,
        argument: null,
        retry: null,
        name: r'coreBootstrapProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$coreBootstrapHash();

  @$internal
  @override
  $ProviderElement<CoreBootstrap> $createElement($ProviderPointer pointer) =>
      $ProviderElement(pointer);

  @override
  CoreBootstrap create(Ref ref) {
    return coreBootstrap(ref);
  }

  /// {@macro riverpod.override_with_value}
  Override overrideWithValue(CoreBootstrap value) {
    return $ProviderOverride(
      origin: this,
      providerOverride: $SyncValueProvider<CoreBootstrap>(value),
    );
  }
}

String _$coreBootstrapHash() => r'0f6fc32890fe7ef76d7cc85906d1f641a58b46e0';

/// Starts the core once: loads the library, then `initCore` with this
/// install's configuration. The result is the session to route on until the
/// session stream emits (the app shows a splash meanwhile).

@ProviderFor(coreStartup)
final coreStartupProvider = CoreStartupProvider._();

/// Starts the core once: loads the library, then `initCore` with this
/// install's configuration. The result is the session to route on until the
/// session stream emits (the app shows a splash meanwhile).

final class CoreStartupProvider
    extends
        $FunctionalProvider<
          AsyncValue<bridge.SessionState>,
          bridge.SessionState,
          FutureOr<bridge.SessionState>
        >
    with
        $FutureModifier<bridge.SessionState>,
        $FutureProvider<bridge.SessionState> {
  /// Starts the core once: loads the library, then `initCore` with this
  /// install's configuration. The result is the session to route on until the
  /// session stream emits (the app shows a splash meanwhile).
  CoreStartupProvider._()
    : super(
        from: null,
        argument: null,
        retry: noCoreRetry,
        name: r'coreStartupProvider',
        isAutoDispose: false,
        dependencies: null,
        $allTransitiveDependencies: null,
      );

  @override
  String debugGetCreateSourceHash() => _$coreStartupHash();

  @$internal
  @override
  $FutureProviderElement<bridge.SessionState> $createElement(
    $ProviderPointer pointer,
  ) => $FutureProviderElement(pointer);

  @override
  FutureOr<bridge.SessionState> create(Ref ref) {
    return coreStartup(ref);
  }
}

String _$coreStartupHash() => r'd9adcc016c5b648dd714fe7f5db50c6e79888856';
