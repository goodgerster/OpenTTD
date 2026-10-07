//! C++ bindings for the Rust parts of OpenTTD.
//!
//! This is the only Rust library linked into the game: every Rust static
//! library carries its own copy of the standard library, so linking two would
//! give duplicate symbols. Other crates are exposed to C++ through this one.
//!
//! The bridge is compiled by CMake into the `openttd_rs_bridge` target. C++
//! code includes `openttd_rs_bridge/lib.h` and calls the functions in
//! namespace `ottd_rs`; keep the C++ wrappers thin.

use openttd_core::math::{int_sqrt_u32, int_sqrt_u64};

#[cxx::bridge(namespace = "ottd_rs")]
mod ffi {
    extern "Rust" {
        /// See `openttd_core::math::int_sqrt_u32`.
        fn int_sqrt_u32(num: u32) -> u32;

        /// See `openttd_core::math::int_sqrt_u64`.
        fn int_sqrt_u64(num: u64) -> u64;
    }
}
