//! Income from delivering cargo.
//!
//! Port of the arithmetic in `GetTransportedGoodsIncome` (`src/economy.cpp`).
//! The C++ side still looks up the cargo, scales the transit time by the
//! `economy.cargo_aging_rate` setting and calls the NewGRF profit callback.
//!
//! Unlike the original, which computed in 32 bits and wrapped for large
//! deliveries over long distances, amounts are computed in 64 bits and the
//! result saturates at the limits of `i64`, like C++ `Money`. Where the
//! original did not overflow, the results are the same.

/// How the payment depends on the transit time (`economy.payment_algorithm`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentAlgorithm {
    /// Transit times are capped at 255 periods.
    Traditional,
    /// Transit times are not capped.
    Modern,
}

/// The payment parameters of a cargo type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CargoPaymentRates {
    /// The two transit time thresholds of the cargo, in cargo aging periods.
    pub transit_periods: [u8; 2],
    /// The current payment rate of the cargo, including inflation.
    pub current_payment: i64,
}

const MIN_TIME_FACTOR: i32 = 31;
const MAX_TIME_FACTOR: i32 = 255;
const TIME_FACTOR_FRAC_BITS: u32 = 4;
const TIME_FACTOR_FRAC: i32 = 1 << TIME_FACTOR_FRAC_BITS;

/// Multiplies an amount by a payment rate, saturating at the limits of `i64`.
///
/// The payment rate is used as a 32-bit value, as in the original; it stays
/// far below 2^31 in practice.
fn multiply_by_payment_rate(amount: i64, payment: i64) -> i64 {
    amount.saturating_mul(i64::from(payment as i32))
}

/// Returns the income for delivering cargo, depending on the distance and the
/// transit time.
///
/// `transit_periods` is the transit time after scaling by the cargo aging rate.
pub fn income_from_transit_time(
    num_pieces: u32,
    distance: u32,
    transit_periods: u16,
    rates: &CargoPaymentRates,
    algorithm: PaymentAlgorithm,
) -> i64 {
    let transit = match algorithm {
        PaymentAlgorithm::Traditional => i32::from(transit_periods.min(0xFF)),
        PaymentAlgorithm::Modern => i32::from(transit_periods),
    };
    let periods1 = i32::from(rates.transit_periods[0]);
    let periods2 = i32::from(rates.transit_periods[1]);
    let periods_over_periods1 = (transit - periods1).max(0);
    let periods_over_periods2 = (periods_over_periods1 - periods2).max(0);
    let mut periods_over_max = MIN_TIME_FACTOR - MAX_TIME_FACTOR;
    if periods2 > -periods_over_max {
        periods_over_max += transit - periods1;
    } else {
        periods_over_max += 2 * (transit - periods1) - periods2;
    }

    // The time factor is constant for fast transits, then decreases linearly
    // with a slope of -1 and then -2, and after reaching MIN_TIME_FACTOR it
    // approaches 1 as a scaled 1/(x+1) function, in fixed point.
    let (time_factor, shift) = if periods_over_max > 0 {
        let factor = (2 * MIN_TIME_FACTOR * TIME_FACTOR_FRAC * TIME_FACTOR_FRAC
            / (periods_over_max + 2 * TIME_FACTOR_FRAC))
            .max(1);
        (factor, 21 + TIME_FACTOR_FRAC_BITS)
    } else {
        let factor =
            (MAX_TIME_FACTOR - periods_over_periods1 - periods_over_periods2).max(MIN_TIME_FACTOR);
        (factor, 21)
    };

    let amount = i64::from(distance)
        .saturating_mul(i64::from(time_factor))
        .saturating_mul(i64::from(num_pieces));
    multiply_by_payment_rate(amount, rates.current_payment) >> shift
}

/// Returns the income for delivering cargo from the result of the NewGRF
/// cargo profit callback.
///
/// The callback returns a 15-bit signed multiplier, which is multiplied by the
/// amount of cargo and the payment rate and then divided by 8192.
pub fn income_from_profit_callback(callback_result: u16, num_pieces: u32, payment: i64) -> i64 {
    let mut multiplier = i64::from(callback_result & 0x3FFF);
    if callback_result & 0x4000 != 0 {
        multiplier -= 0x4000;
    }
    multiply_by_payment_rate(multiplier * i64::from(num_pieces), payment) / 8192
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: CargoPaymentRates = CargoPaymentRates {
        transit_periods: [7, 24],
        current_payment: 5916,
    };

    fn modern(num_pieces: u32, distance: u32, transit: u16, rates: &CargoPaymentRates) -> i64 {
        income_from_transit_time(
            num_pieces,
            distance,
            transit,
            rates,
            PaymentAlgorithm::Modern,
        )
    }

    #[test]
    fn fast_delivery_has_the_full_time_factor() {
        // 20 * 255 * 10 * 5916 >> 21
        assert_eq!(modern(10, 20, 0, &RATES), 143);
        assert_eq!(modern(10, 20, 7, &RATES), 143);
    }

    #[test]
    fn time_factor_decreases_after_the_first_threshold() {
        // Time factor 255 - 10 = 245.
        assert_eq!(modern(10, 20, 17, &RATES), 138);
        // Time factor 255 - 33 - 9 = 213.
        assert_eq!(modern(10, 20, 40, &RATES), 120);
    }

    #[test]
    fn slow_delivery_uses_a_fixed_point_time_factor() {
        // 2 * 193 - 24 - 224 = 138 periods over; time factor 15872 / 170 = 93, shift 25.
        assert_eq!(modern(100, 500, 200, &RATES), 819);
        let rates = CargoPaymentRates {
            transit_periods: [0, 24],
            ..RATES
        };
        assert_eq!(modern(100, 500, 300, &rates), 361);
        assert_eq!(modern(1000, 1000, 1000, &rates), 1410);
    }

    #[test]
    fn traditional_algorithm_caps_the_transit_time() {
        let rates = CargoPaymentRates {
            transit_periods: [0, 24],
            ..RATES
        };
        let traditional =
            income_from_transit_time(1000, 1000, 1000, &rates, PaymentAlgorithm::Traditional);
        assert_eq!(traditional, 9344);
        assert_eq!(traditional, modern(1000, 1000, 255, &rates));
    }

    #[test]
    fn negative_payment_rounds_down() {
        let rates = CargoPaymentRates {
            current_payment: -3000,
            ..RATES
        };
        // -153000000 / 2^21 = -72.96, rounded towards minus infinity.
        assert_eq!(modern(10, 20, 0, &rates), -73);
    }

    #[test]
    fn large_deliveries_do_not_wrap() {
        let rates = CargoPaymentRates {
            transit_periods: [0, 24],
            ..RATES
        };
        // 4200 * 255 * 2100 is more than 2^31.
        assert_eq!(modern(2100, 4200, 0, &rates), 6_344_640);
        assert_eq!(
            modern(60000, 60000, 0, &rates),
            (60000_i64 * 255 * 60000 * 5916) >> 21
        );
    }

    #[test]
    fn extreme_amounts_saturate() {
        let rates = CargoPaymentRates {
            transit_periods: [0, 24],
            ..RATES
        };
        assert_eq!(modern(u32::MAX, u32::MAX, 0, &rates), i64::MAX >> 21);
        let negative = CargoPaymentRates {
            current_payment: -1,
            ..rates
        };
        assert_eq!(modern(u32::MAX, u32::MAX, 0, &negative), i64::MIN >> 21);
    }

    #[test]
    fn payment_rate_is_used_as_32_bits() {
        let rates = CargoPaymentRates {
            current_payment: (1_i64 << 32) + 5916,
            ..RATES
        };
        assert_eq!(modern(10, 20, 0, &rates), modern(10, 20, 0, &RATES));
    }

    #[test]
    fn profit_callback_multiplier_is_15_bit_signed() {
        let minus_100 = 0x4000 | (0x4000 - 100);
        assert_eq!(income_from_profit_callback(minus_100, 10, 8192), -1000);
        assert_eq!(income_from_profit_callback(minus_100, 1, 4096), -50);
        assert_eq!(income_from_profit_callback(0x4000, 1, 8192), -0x4000);
        assert_eq!(income_from_profit_callback(100, 10, 8192), 1000);
        // Bit 15 is not part of the result.
        assert_eq!(income_from_profit_callback(0x8000 | 100, 10, 8192), 1000);
    }

    #[test]
    fn profit_callback_division_rounds_towards_zero() {
        assert_eq!(income_from_profit_callback(1, 1, 8191), 0);
        assert_eq!(income_from_profit_callback(0x7FFF, 1, 8191), 0);
    }

    #[test]
    fn large_profit_callback_results_do_not_wrap() {
        assert_eq!(
            income_from_profit_callback(0x3FFF, 300_000, 8192),
            0x3FFF * 300_000
        );
    }

    /// The original 32-bit computation, with the wrapping it relied on.
    fn original_transit_time_income(
        num_pieces: u32,
        distance: u32,
        transit: u16,
        rates: &CargoPaymentRates,
    ) -> i64 {
        let periods1 = i32::from(rates.transit_periods[0]);
        let periods2 = i32::from(rates.transit_periods[1]);
        let transit = i32::from(transit);
        let over1 = (transit - periods1).max(0);
        let over2 = (over1 - periods2).max(0);
        let mut over_max = MIN_TIME_FACTOR - MAX_TIME_FACTOR;
        if periods2 > -over_max {
            over_max += transit - periods1;
        } else {
            over_max += 2 * (transit - periods1) - periods2;
        }
        let big_mul_s =
            |a: i32, b: i32, shift: u32| ((i64::from(a) * i64::from(b)) >> shift) as i32;
        let (factor, shift) = if over_max > 0 {
            ((2 * MIN_TIME_FACTOR * 256 / (over_max + 32)).max(1), 25)
        } else {
            ((MAX_TIME_FACTOR - over1 - over2).max(MIN_TIME_FACTOR), 21)
        };
        let a = distance
            .wrapping_mul(factor as u32)
            .wrapping_mul(num_pieces) as i32;
        i64::from(big_mul_s(a, rates.current_payment as i32, shift))
    }

    #[test]
    fn matches_the_original_where_it_did_not_overflow() {
        for periods in [[0, 24], [7, 255], [255, 255], [40, 0]] {
            for payment in [1, 4778, 100_000, -3000] {
                let rates = CargoPaymentRates {
                    transit_periods: periods,
                    current_payment: payment,
                };
                for distance in [0, 1, 20, 200, 1000, 4000] {
                    for num_pieces in [0, 1, 10, 333, 1000] {
                        for transit in [0, 1, 7, 30, 100, 255, 300, 1000, 65535] {
                            assert_eq!(
                                modern(num_pieces, distance, transit, &rates),
                                original_transit_time_income(num_pieces, distance, transit, &rates),
                                "{periods:?} {payment} {distance} {num_pieces} {transit}"
                            );
                        }
                    }
                }
            }
        }
    }
}
