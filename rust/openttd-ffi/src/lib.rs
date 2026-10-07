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
use openttd_core::cargo_income::{self, CargoPaymentRates, PaymentAlgorithm};
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

        /// Income for delivering cargo, from distance and transit time. `traditional`
        /// selects the traditional payment algorithm. See
        /// `openttd_core::cargo_income::income_from_transit_time`.
        fn cargo_income_from_transit_time(
            num_pieces: u32,
            distance: u32,
            transit_periods: u16,
            cargo_transit_periods_1: u8,
            cargo_transit_periods_2: u8,
            current_payment: i64,
            traditional: bool,
        ) -> i64;

        /// Income for delivering cargo from the result of the NewGRF cargo profit
        /// callback. See `openttd_core::cargo_income::income_from_profit_callback`.
        fn cargo_income_from_profit_callback(
            callback_result: u16,
            num_pieces: u32,
            payment: i64,
        ) -> i64;
    }
}

fn cargo_income_from_transit_time(
    num_pieces: u32,
    distance: u32,
    transit_periods: u16,
    cargo_transit_periods_1: u8,
    cargo_transit_periods_2: u8,
    current_payment: i64,
    traditional: bool,
) -> i64 {
    let rates = CargoPaymentRates {
        transit_periods: [cargo_transit_periods_1, cargo_transit_periods_2],
        current_payment,
    };
    let algorithm = if traditional {
        PaymentAlgorithm::Traditional
    } else {
        PaymentAlgorithm::Modern
    };
    cargo_income::income_from_transit_time(num_pieces, distance, transit_periods, &rates, algorithm)
}

fn cargo_income_from_profit_callback(callback_result: u16, num_pieces: u32, payment: i64) -> i64 {
    cargo_income::income_from_profit_callback(callback_result, num_pieces, payment)
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
