# Builds the Rust client core (client/core, crate `strata-core`, library `strata_core`) with
# cargokit and bundles it with the app (Flutter FFI plugin).
cmake_minimum_required(VERSION 3.14)

set(PROJECT_NAME "strata_bridge")
project(${PROJECT_NAME} LANGUAGES CXX)

# The app reaches this file through Flutter's plugin symlink
# (…/flutter/ephemeral/.plugin_symlinks/strata_bridge) and CMake resolves `..` lexically, so
# the include path starts from the real directory. (cargokit resolves the manifest path
# below itself.)
get_filename_component(STRATA_BRIDGE_DIR "${CMAKE_CURRENT_SOURCE_DIR}" REALPATH)
include("${STRATA_BRIDGE_DIR}/../../../../core/cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../../../../core strata_core "")

set(strata_bridge_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  PARENT_SCOPE
)
