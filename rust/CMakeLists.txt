# Rust parts of OpenTTD, built with Cargo through Corrosion (cmake/3rdparty/corrosion).
# Only the openttd-ffi crate is linked into the game; see rust/openttd-ffi/src/lib.rs.

# The first rustc in PATH is used. If that is rustup's, select the toolchain
# pinned in rust-toolchain.toml explicitly, so that build directories outside
# the source tree (where rustup would not find that file) use it too. Rust from
# the Nix dev shell (flake.nix) is already the pinned version.
find_program(RUSTC_IN_PATH rustc)
find_program(RUSTUP_EXECUTABLE rustup PATHS "$ENV{HOME}/.cargo/bin")
set(RUST_TEST_ENV)
if(RUSTC_IN_PATH AND RUSTUP_EXECUTABLE)
    get_filename_component(RUSTC_DIR "${RUSTC_IN_PATH}" DIRECTORY)
    get_filename_component(RUSTUP_DIR "${RUSTUP_EXECUTABLE}" DIRECTORY)
endif()
if(RUSTUP_EXECUTABLE AND (NOT RUSTC_IN_PATH OR RUSTC_DIR STREQUAL RUSTUP_DIR))
    file(STRINGS "${CMAKE_SOURCE_DIR}/rust-toolchain.toml" RUST_TOOLCHAIN_CHANNEL REGEX "^channel *= *\"[^\"]+\"")
    string(REGEX REPLACE "^channel *= *\"([^\"]+)\".*" "\\1" Rust_TOOLCHAIN "${RUST_TOOLCHAIN_CHANNEL}")
    set(RUST_TEST_ENV "RUSTUP_TOOLCHAIN=${Rust_TOOLCHAIN}")
endif()

# Corrosion requires CMake 3.22; add_subdirectory() keeps its policy settings out of the rest of the project.
add_subdirectory(${CMAKE_SOURCE_DIR}/cmake/3rdparty/corrosion ${CMAKE_BINARY_DIR}/corrosion_subproject)

corrosion_import_crate(
    MANIFEST_PATH Cargo.toml
    CRATES openttd-ffi
    CRATE_TYPES staticlib
    LOCKED
)

# C++ side of the cxx bridge. C++ code includes "openttd_rs_bridge/lib.h".
corrosion_add_cxxbridge(openttd_rs_bridge CRATE openttd_ffi FILES lib.rs)

# Rust unit tests, run as part of ctest. The first run compiles the test
# binaries, hence the longer timeout.
add_test(NAME rust_cargo_test
    COMMAND ${CMAKE_COMMAND} -E env
            ${RUST_TEST_ENV}
            "CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_test"
            "${Rust_CARGO_CACHED}" test --workspace --locked
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(rust_cargo_test PROPERTIES TIMEOUT 600)
