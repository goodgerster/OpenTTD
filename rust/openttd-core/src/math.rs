//! Integer maths helpers, ported from `src/core/math_func.cpp`.

/// Defines an integer square root function for one unsigned type.
///
/// Mirrors the C++ template `IntSqrtImplementation`, which `IntSqrt` and
/// `IntSqrt64` instantiate, so both widths share one algorithm.
macro_rules! int_sqrt {
    ($(#[$doc:meta])* $name:ident, $t:ty) => {
        $(#[$doc])*
        pub fn $name(mut num: $t) -> $t {
            if num <= 1 {
                return num;
            }

            // `bit` starts at the highest power of four <= the argument.
            let leading_zeros = num.leading_zeros() | 1;
            let mut bit: $t = 1 << (<$t>::BITS - leading_zeros - 1);
            let mut res: $t = 0;

            while bit != 0 {
                if num >= res + bit {
                    num -= res + bit;
                    res = (res >> 1) + bit;
                } else {
                    res >>= 1;
                }
                bit >>= 2;
            }

            // Round to the nearest integer.
            if num > res {
                res += 1;
            }
            res
        }
    };
}

int_sqrt!(
    /// Computes the square root of `num`, rounded to the nearest integer.
    ///
    /// Port of `IntSqrt` (C++). Uses only integer arithmetic, so the result
    /// is identical on every platform.
    int_sqrt_u32,
    u32
);

int_sqrt!(
    /// Computes the square root of `num`, rounded to the nearest integer.
    ///
    /// Port of `IntSqrt64` (C++). Uses only integer arithmetic, so the result
    /// is identical on every platform.
    int_sqrt_u64,
    u64
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `r` is `sqrt(n)` rounded to the nearest integer, i.e. `(2r - 1)^2 < 4n < (2r + 1)^2`.
    /// The bounds cannot be equal, as `4n` is even and `(2r +- 1)^2` is odd.
    fn is_rounded_sqrt(n: u64, r: u64) -> bool {
        let four_n = 4 * u128::from(n);
        let r = u128::from(r);
        let below = r == 0 || (2 * r - 1).pow(2) < four_n;
        below && four_n < (2 * r + 1).pow(2)
    }

    /// Deterministic pseudo-random sequence (xorshift64) for sampling large input ranges.
    fn xorshift_samples(count: usize) -> impl Iterator<Item = u64> {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        std::iter::repeat_with(move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        })
        .take(count)
    }

    /// The cases from `IntSqrtTest` in `src/tests/math_func.cpp`.
    #[test]
    fn int_sqrt_u32_cpp_cases() {
        assert_eq!(int_sqrt_u32(0), 0);
        assert_eq!(int_sqrt_u32(1), 1);
        assert_eq!(int_sqrt_u32(2), 1);
        assert_eq!(int_sqrt_u32(3), 2);
        assert_eq!(int_sqrt_u32(4), 2);
        assert_eq!(int_sqrt_u32(25), 5);
        assert_eq!(int_sqrt_u32(100), 10);
        assert_eq!(int_sqrt_u32(88), 9);
        assert_eq!(int_sqrt_u32(2_876_278), 1696);
        assert_eq!(int_sqrt_u32(u32::MAX), 0x1_0000);
    }

    /// The cases from `IntSqrt64Test` in `src/tests/math_func.cpp`.
    #[test]
    fn int_sqrt_u64_cpp_cases() {
        assert_eq!(int_sqrt_u64(1), 1);
        assert_eq!(int_sqrt_u64(2), 1);
        assert_eq!(int_sqrt_u64(3), 2);
        assert_eq!(int_sqrt_u64(4), 2);
        assert_eq!(int_sqrt_u64(25), 5);
        assert_eq!(int_sqrt_u64(100), 10);
        assert_eq!(int_sqrt_u64(88), 9);
        assert_eq!(int_sqrt_u64(2_876_278), 1696);
        assert_eq!(int_sqrt_u64(u64::from(u32::MAX)), 0x1_0000);
        assert_eq!(int_sqrt_u64(u64::MAX), 0x1_0000_0000);
    }

    /// Rounding switches between `r^2 + r` (rounds down) and `r^2 + r + 1` (rounds up).
    #[test]
    fn rounding_boundaries() {
        for r in (1..=0xFFFFu64).chain([0xFFFF_FFFE, 0xFFFF_FFFF]) {
            let n = r * r + r;
            if let Ok(n32) = u32::try_from(n + 1) {
                assert_eq!(int_sqrt_u32(n32 - 1), r as u32, "n = {}", n);
                assert_eq!(int_sqrt_u32(n32), r as u32 + 1, "n = {}", n + 1);
            }
            assert_eq!(int_sqrt_u64(n), r, "n = {n}");
            assert_eq!(int_sqrt_u64(n + 1), r + 1, "n = {}", n + 1);
        }
    }

    #[test]
    fn int_sqrt_u32_small_inputs_exhaustive() {
        for n in 0..=0x2_0000u32 {
            assert!(is_rounded_sqrt(n.into(), int_sqrt_u32(n).into()), "n = {n}");
        }
    }

    #[test]
    fn sampled_inputs_are_rounded_sqrt() {
        for n in xorshift_samples(100_000) {
            assert!(is_rounded_sqrt(n, int_sqrt_u64(n)), "n = {n}");
            let n32 = n as u32;
            assert!(
                is_rounded_sqrt(n32.into(), int_sqrt_u32(n32).into()),
                "n = {n32}"
            );
        }
    }
}
