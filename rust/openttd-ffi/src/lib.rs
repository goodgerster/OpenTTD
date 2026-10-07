//! C++ bindings for the Rust parts of OpenTTD.
//!
//! This is the only Rust library linked into the game: every Rust static
//! library carries its own copy of the standard library, so linking two would
//! give duplicate symbols. Other crates are exposed to C++ through this one.
//!
//! The bridge is compiled by CMake into the `openttd_rs_bridge` target. C++
//! code includes `openttd_rs_bridge/lib.h` and calls the functions in
//! namespace `ottd_rs`; keep the C++ wrappers thin.

use openttd_core::bmp;
use openttd_core::math::{int_sqrt_u32, int_sqrt_u64};

#[cxx::bridge(namespace = "ottd_rs")]
mod ffi {
    /// Header information of a BMP file; see `openttd_core::bmp::BmpInfo`.
    struct BmpInfo {
        offset: u64,
        width: u32,
        height: u32,
        os2_bmp: bool,
        bpp: u16,
        compression: u32,
        palette_size: u32,
    }

    /// A palette entry of a BMP file.
    struct BmpColour {
        r: u8,
        g: u8,
        b: u8,
    }

    extern "Rust" {
        /// See `openttd_core::math::int_sqrt_u32`.
        fn int_sqrt_u32(num: u32) -> u32;

        /// See `openttd_core::math::int_sqrt_u64`.
        fn int_sqrt_u64(num: u64) -> u64;

        /// Reads the header and palette of the BMP file `file`; returns false if
        /// it is not a BMP in a supported format. See `openttd_core::bmp::read_header`.
        fn bmp_read_header(file: &[u8], info: &mut BmpInfo, palette: &mut Vec<BmpColour>) -> bool;

        /// Decodes the bitmap of `file` into `bitmap`; returns false if the data
        /// is truncated or invalid. See `openttd_core::bmp::read_bitmap`.
        fn bmp_read_bitmap(file: &[u8], info: &BmpInfo, bitmap: &mut [u8]) -> bool;
    }
}

fn bmp_read_header(
    file: &[u8],
    info: &mut ffi::BmpInfo,
    palette: &mut Vec<ffi::BmpColour>,
) -> bool {
    let Some(header) = bmp::read_header(file) else {
        return false;
    };
    let bmp::BmpInfo {
        offset,
        width,
        height,
        os2_bmp,
        bpp,
        compression,
        palette_size,
    } = header.info;
    *info = ffi::BmpInfo {
        offset,
        width,
        height,
        os2_bmp,
        bpp,
        compression,
        palette_size,
    };
    palette.clear();
    palette.extend(header.palette.iter().map(|c| ffi::BmpColour {
        r: c.r,
        g: c.g,
        b: c.b,
    }));
    true
}

fn bmp_read_bitmap(file: &[u8], info: &ffi::BmpInfo, bitmap: &mut [u8]) -> bool {
    let info = bmp::BmpInfo {
        offset: info.offset,
        width: info.width,
        height: info.height,
        os2_bmp: info.os2_bmp,
        bpp: info.bpp,
        compression: info.compression,
        palette_size: info.palette_size,
    };
    bmp::read_bitmap(file, &info, bitmap)
}
