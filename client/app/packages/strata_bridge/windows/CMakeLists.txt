# Builds the Rust client core (client/core, crate `strata-core`, library `strata_core`) with
# cargokit and bundles it with the app (Flutter FFI plugin).
cmake_minimum_required(VERSION 3.14)

set(PROJECT_NAME "strata_bridge")
project(${PROJECT_NAME} LANGUAGES CXX)

include("../cargokit/cmake/cargokit.cmake")
apply_cargokit(${PROJECT_NAME} ../../../../core strata_core "")

set(strata_bridge_bundled_libraries
  "${${PROJECT_NAME}_cargokit_lib}"
  PARENT_SCOPE
)
