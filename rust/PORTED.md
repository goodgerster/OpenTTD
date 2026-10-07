# Ported C++ code

C++ functions whose implementation now lives in Rust. When a merge from
upstream OpenTTD or jgrpp changes one of these:

1. Update the C++ reference implementation in the listed test file to match the new upstream version.
2. Make the same change in Rust, test-first.
3. Keep the C++ function a thin wrapper around the Rust one.

The merge may show the change as a conflict in the C++ file (where the
original implementation was replaced) or as a change to the test file.

| C++ function | C++ file | Rust function | C++ reference kept in |
| --- | --- | --- | --- |
| `IntSqrt`, `IntSqrt64` | `src/core/math_func.cpp` | `openttd_core::math::int_sqrt_u32`, `int_sqrt_u64` | `src/tests/math_func.cpp` |
| `BmpReadHeader`, `BmpReadBitmap` | `src/bmp.cpp` | `openttd_core::bmp::read_header`, `read_bitmap` | `src/tests/bmp.cpp` |
| `GetTransportedGoodsIncome` (arithmetic only; the cargo lookup, transit time scaling and NewGRF callback stay in C++) | `src/economy.cpp` | `openttd_core::cargo_income::income_from_transit_time`, `income_from_profit_callback` | `src/tests/cargo_income.cpp` |

## Deliberate differences

- **BMP:**
  - The C++ wrappers read the whole file into memory and Rust decodes the bytes.
  - For a BMP inside a tar file, reading past the BMP's end now yields zeros; the original read the following bytes of the tar file.
  - Where the original has undefined behaviour or aborts, the port returns failure instead: RLE images with a height of 0, and header values that its header check rejects. Neither can happen through `heightmap.cpp`.
  - **Bug fix (also present upstream):** uncompressed 1 bpp and 4 bpp images whose rows end in a partly used byte (width not a multiple of 8 or 2) are now decoded correctly. The original computes the row padding from `width / 8` or `width / 2` rounded down, so it reads every row after the first one byte late. Such heightmaps now give different maps than in jgrpp and upstream. The differential test in `src/tests/bmp.cpp` skips these images; the known-answer tests there and in `openttd-core/src/bmp.rs` cover them.
- **Cargo income:**
  - **Bug fix:** amounts are computed in 64 bits and saturate. The original computed `distance * time_factor * amount` in 32 bits and cut the result to 32 bits, and with the profit callback multiplied the multiplier by the amount in 32 bits, so large deliveries wrapped into wrong, usually negative, incomes. Results are the same wherever the original did not overflow; the differential test in `src/tests/cargo_income.cpp` checks this.
  - The payment rate is used as a 32-bit value, as in the original.
