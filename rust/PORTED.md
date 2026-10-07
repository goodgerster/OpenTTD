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
