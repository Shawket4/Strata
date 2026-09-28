#
# Builds the Rust client core (client/core, library `strata_core`) with cargokit as a static
# library and force-loads it into the plugin.
#
Pod::Spec.new do |s|
  s.name             = 'strata_bridge'
  s.version          = '0.1.0'
  s.summary          = 'Strata Rust client core bridge.'
  s.description      = 'flutter_rust_bridge bindings of the Strata Rust client core.'
  s.homepage         = 'https://strata.invalid'
  s.license          = { :type => 'UNLICENSED' }
  s.author           = { 'Strata' => 'strata@strata.invalid' }
  s.module_name      = 'strata_bridge'
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'Flutter'
  s.platform = :ios, '12.0'
  s.swift_version    = '5.0'

  s.script_phase = {
    :name => 'Build Rust library',
    # Flutter reaches the plugin through a symlink (.symlinks/plugins/strata_bridge), and `..`
    # from there leaves the plugin; the physical path reaches client/core.
    :script => 'PODS_TARGET_SRCROOT="$(cd "$PODS_TARGET_SRCROOT" && pwd -P)"; export PODS_TARGET_SRCROOT; ' \
               'sh "$PODS_TARGET_SRCROOT/../../../../core/cargokit/build_pod.sh" ../../../../core strata_core',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    :output_files => ["${PODS_CONFIGURATION_BUILD_DIR}/strata_bridge/libstrata_core.a"],
  }
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'OTHER_LDFLAGS' => '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/strata_bridge/libstrata_core.a',
  }
end
