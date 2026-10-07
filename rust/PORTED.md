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

## Deliberate differences

- **BMP:**
  - The C++ wrappers read the whole file into memory and Rust decodes the bytes.
  - For a BMP inside a tar file, reading past the BMP's end now yields zeros; the original read the following bytes of the tar file.
  - Where the original has undefined behaviour or aborts, the port returns failure instead: RLE images with a height of 0, and header values that its header check rejects. Neither can happen through `heightmap.cpp`.

## Known bugs kept from the original

- **BMP:** uncompressed 1 bpp and 4 bpp images whose rows are not a whole number of bytes are decoded wrongly. The row padding is computed from `width / 8` or `width / 2` rounded down instead of the bytes per row, so every row after the first starts one byte late. The test `pads_1bpp_rows_like_the_original` in `openttd-core/src/bmp.rs` pins this.
