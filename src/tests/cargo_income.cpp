/*
 * This file is part of OpenTTD.
 * OpenTTD is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 2.
 * OpenTTD is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 * See the GNU General Public License for more details. You should have received a copy of the GNU General Public License along with OpenTTD. If not, see <https://www.gnu.org/licenses/old-licenses/gpl-2.0>.
 */

/** @file cargo_income.cpp Tests for the income from delivering cargo. */

#include "../stdafx.h"

#include "../3rdparty/catch2/catch.hpp"

#include "../cargotype.h"
#include "../economy_func.h"
#include "../settings_type.h"
#include "../core/bitmath_func.hpp"
#include "../core/math_func.hpp"

#include "openttd_rs_bridge/lib.h"

#include "../safeguards.h"

/**
 * The original C++ implementation, which computes in 32 bits. It is kept to
 * check that the Rust port (openttd_core::cargo_income, see rust/PORTED.md)
 * gives the same results wherever the original neither overflowed nor wrapped.
 */
namespace cpp_reference {

static inline int32_t BigMulS(const int32_t a, const int32_t b, const uint8_t shift)
{
	return (int32_t)((int64_t)a * (int64_t)b >> shift);
}

static Money GetTransportedGoodsIncome(uint num_pieces, uint dist, uint16_t transit_periods, const CargoSpec *cs)
{
	transit_periods = ScaleByPercentage<uint16_t, uint32_t>(transit_periods, _settings_game.economy.cargo_aging_rate);

	static const int MIN_TIME_FACTOR = 31;
	static const int MAX_TIME_FACTOR = 255;
	static const int TIME_FACTOR_FRAC_BITS = 4;
	static const int TIME_FACTOR_FRAC = 1 << TIME_FACTOR_FRAC_BITS;

	if (_settings_game.economy.payment_algorithm == CPA_TRADITIONAL) transit_periods = std::min<uint16_t>(transit_periods, 0xFFu);

	const int periods1 = cs->transit_periods[0];
	const int periods2 = cs->transit_periods[1];
	const int periods_over_periods1 = std::max(transit_periods - periods1, 0);
	const int periods_over_periods2 = std::max(periods_over_periods1 - periods2, 0);
	int periods_over_max = MIN_TIME_FACTOR - MAX_TIME_FACTOR;
	if (periods2 > -periods_over_max) {
		periods_over_max += transit_periods - periods1;
	} else {
		periods_over_max += 2 * (transit_periods - periods1) - periods2;
	}

	if (periods_over_max > 0) {
		const int time_factor = std::max(2 * MIN_TIME_FACTOR * TIME_FACTOR_FRAC * TIME_FACTOR_FRAC / (periods_over_max + 2 * TIME_FACTOR_FRAC), 1);
		return BigMulS(dist * time_factor * num_pieces, cs->current_payment, 21 + TIME_FACTOR_FRAC_BITS);
	} else {
		const int time_factor = std::max(MAX_TIME_FACTOR - periods_over_periods1 - periods_over_periods2, MIN_TIME_FACTOR);
		return BigMulS(dist * time_factor * num_pieces, cs->current_payment, 21);
	}
}

static Money GetCargoProfitCallbackIncome(uint16_t callback, uint num_pieces, Money payment)
{
	int result = GB(callback, 0, 14);
	if (HasBit(callback, 14)) result -= 0x4000;
	return result * num_pieces * payment / 8192;
}

} // namespace cpp_reference

/** Sets up cargo type 0 for the income calculation, and restores it and the settings afterwards. */
struct TestCargo {
	static constexpr CargoType TYPE{0};

	CargoSpec *cs = CargoSpec::Get(TYPE);
	const uint8_t old_bitnum = cs->bitnum;
	const uint8_t old_transit_periods[2] = { cs->transit_periods[0], cs->transit_periods[1] };
	const Money old_payment = cs->current_payment;
	const CargoCallbackMasks old_callback_mask = cs->callback_mask;
	const uint8_t old_aging_rate = _settings_game.economy.cargo_aging_rate;
	const CargoPaymentAlgorithm old_algorithm = _settings_game.economy.payment_algorithm;

	TestCargo(uint8_t periods1, uint8_t periods2, Money payment, CargoPaymentAlgorithm algorithm)
	{
		this->cs->bitnum = 0;
		this->cs->transit_periods[0] = periods1;
		this->cs->transit_periods[1] = periods2;
		this->cs->current_payment = payment;
		this->cs->callback_mask = {};
		_settings_game.economy.cargo_aging_rate = 100;
		_settings_game.economy.payment_algorithm = algorithm;
	}

	~TestCargo()
	{
		this->cs->bitnum = this->old_bitnum;
		this->cs->transit_periods[0] = this->old_transit_periods[0];
		this->cs->transit_periods[1] = this->old_transit_periods[1];
		this->cs->current_payment = this->old_payment;
		this->cs->callback_mask = this->old_callback_mask;
		_settings_game.economy.cargo_aging_rate = this->old_aging_rate;
		_settings_game.economy.payment_algorithm = this->old_algorithm;
	}
};

TEST_CASE("Cargo income - matches the original implementation without overflow")
{
	/* With these limits dist * time_factor * num_pieces stays below 2^31 and the result fits in 32 bits. */
	for (CargoPaymentAlgorithm algorithm : { CPA_TRADITIONAL, CPA_MODERN }) {
		for (auto [periods1, periods2] : std::initializer_list<std::pair<uint8_t, uint8_t>>{ { 0, 24 }, { 7, 255 }, { 255, 255 }, { 40, 0 } }) {
			for (Money payment : { Money(1), Money(4778), Money(100000), Money(-3000) }) {
				TestCargo cargo(periods1, periods2, payment, algorithm);
				for (uint dist : { 0u, 1u, 20u, 200u, 1000u, 4000u }) {
					for (uint num_pieces : { 0u, 1u, 10u, 333u, 1000u }) {
						for (uint16_t transit : { 0, 1, 7, 30, 100, 255, 300, 1000, 65535 }) {
							INFO("algorithm " << (int)algorithm << ", periods " << (int)periods1 << "/" << (int)periods2 << ", payment " << payment.base()
									<< ", dist " << dist << ", pieces " << num_pieces << ", transit " << transit);
							CHECK(GetTransportedGoodsIncome(num_pieces, dist, transit, TestCargo::TYPE) == cpp_reference::GetTransportedGoodsIncome(num_pieces, dist, transit, cargo.cs));
						}
					}
				}
			}
		}
	}
}

TEST_CASE("Cargo income - large deliveries over long distances")
{
	TestCargo cargo(0, 24, Money(5916), CPA_MODERN);

	/* 4200 * 255 * 2100 is more than 2^31; the original computation wrapped and gave a negative income. */
	CHECK(GetTransportedGoodsIncome(2100, 4200, 0, TestCargo::TYPE).base() == (int64_t{4200} * 255 * 2100 * 5916) >> 21);

	/* An income that doesn't fit in 32 bits. */
	CHECK(GetTransportedGoodsIncome(60000, 60000, 0, TestCargo::TYPE).base() == (int64_t{60000} * 255 * 60000 * 5916) >> 21);

	/* Slow delivery: with transit periods 0 and 24, a transit time of 249 is 2 * 249 - 24 - 224 = 250 periods
	 * past the point where the time factor reaches its minimum, and the time factor becomes a fixed-point
	 * number with 4 fractional bits. */
	const int time_factor = 2 * 31 * 16 * 16 / (250 + 2 * 16);
	CHECK(GetTransportedGoodsIncome(30000, 30000, 249, TestCargo::TYPE).base() == (int64_t{30000} * time_factor * 30000 * 5916) >> 25);
}

TEST_CASE("Cargo income - profit callback")
{
	/* Bit 14 is the sign bit of the 15 bit result. Negative multipliers also worked in the original:
	 * the product wrapped around as an unsigned value, but Money's multiplication truncates its
	 * factor to int, which undid that. */
	const uint16_t minus_100 = 0x4000 | (0x4000 - 100);
	CHECK(ottd_rs::cargo_income_from_profit_callback(minus_100, 10, 8192) == -1000);
	CHECK(ottd_rs::cargo_income_from_profit_callback(minus_100, 1, 4096) == -50);
	CHECK(ottd_rs::cargo_income_from_profit_callback(0x4000, 1, 8192) == -0x4000);

	/* Large positive results no longer wrap. */
	CHECK(ottd_rs::cargo_income_from_profit_callback(0x3FFF, 300000, 8192) == int64_t{0x3FFF} * 300000);

	/* Unchanged where the original did not wrap. */
	for (uint16_t callback : { 0, 1, 100, 0x1000, 0x3FFF }) {
		for (uint num_pieces : { 0u, 1u, 7u, 1000u, 65535u }) {
			for (Money payment : { Money(0), Money(1), Money(4778), Money(8192), Money(123456) }) {
				INFO("callback " << callback << ", pieces " << num_pieces << ", payment " << payment.base());
				CHECK(ottd_rs::cargo_income_from_profit_callback(callback, num_pieces, payment.base()) == cpp_reference::GetCargoProfitCallbackIncome(callback, num_pieces, payment).base());
			}
		}
	}
}
