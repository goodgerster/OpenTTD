/*
 * This file is part of OpenTTD.
 * OpenTTD is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 2.
 * OpenTTD is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 * See the GNU General Public License for more details. You should have received a copy of the GNU General Public License along with OpenTTD. If not, see <https://www.gnu.org/licenses/old-licenses/gpl-2.0>.
 */

/** @file math_func.cpp Test functionality from core/math_func. */

#include "../stdafx.h"

#include "../3rdparty/catch2/catch.hpp"

#include "../core/math_func.hpp"

#include <bit>

#include "../safeguards.h"

/** Original C++ implementations of functions ported to Rust, used to check the ports against. */
namespace cpp_reference {

/**
 * Original C++ implementation of IntSqrt and IntSqrt64, ported to rust/openttd-core/src/math.rs.
 * @param num Radicand.
 * @return Rounded integer square root.
 */
template <typename T>
T IntSqrtImplementation(T num)
{
	if (num <= 1) return num;

	/* 'bit' starts at the highest power of four <= the argument. */
	uint8_t leading_zeroes = std::countl_zero<T>(num) | 1;
	T bit = static_cast<T>(1) << (std::numeric_limits<T>::digits - leading_zeroes - 1);

	T res = 0;
	while (bit != 0) {
		if (num >= res + bit) {
			num -= res + bit;
			res = (res >> 1) + bit;
		} else {
			res >>= 1;
		}
		bit >>= 2;
	}

	/* Arithmetic rounding to nearest integer. */
	if (num > res) res++;

	return res;
}

} // namespace cpp_reference

TEST_CASE("DivideApproxTest - Negative")
{
	CHECK(-2 == DivideApprox(-5, 2));
	CHECK(2 == DivideApprox(-5, -2));
	CHECK(-1 == DivideApprox(-66, 80));
}

TEST_CASE("DivideApproxTest, Divide")
{
	CHECK(2 == DivideApprox(5, 2));
	CHECK(3 == DivideApprox(80, 30));
	CHECK(3 == DivideApprox(8, 3));
	CHECK(0 == DivideApprox(3, 8));
}

TEST_CASE("IntSqrtTest - Zero")
{
	CHECK(0 == IntSqrt(0));
}

TEST_CASE("IntSqrtTest - FindSqRt")
{
	CHECK(1 == IntSqrt(1));
	CHECK(1 == IntSqrt(2));
	CHECK(2 == IntSqrt(3));
	CHECK(2 == IntSqrt(4));
	CHECK(5 == IntSqrt(25));
	CHECK(10 == IntSqrt(100));
	CHECK(9 == IntSqrt(88));
	CHECK(1696 == IntSqrt(2876278));
	CHECK(0x10000 == IntSqrt(std::numeric_limits<uint32_t>::max()));
}

TEST_CASE("IntSqrt64Test - FindSqRt")
{
	CHECK(1 == IntSqrt64(1));
	CHECK(1 == IntSqrt64(2));
	CHECK(2 == IntSqrt64(3));
	CHECK(2 == IntSqrt64(4));
	CHECK(5 == IntSqrt64(25));
	CHECK(10 == IntSqrt64(100));
	CHECK(9 == IntSqrt64(88));
	CHECK(1696 == IntSqrt64(2876278));
	CHECK(0x10000 == IntSqrt64(std::numeric_limits<uint32_t>::max()));
	CHECK(0x100000000ULL == IntSqrt64(std::numeric_limits<uint64_t>::max()));
}

TEST_CASE("IntSqrt - matches C++ reference implementation")
{
	uint64_t mismatches = 0;
	auto check = [&](uint64_t num) {
		if (IntSqrt64(num) != cpp_reference::IntSqrtImplementation<uint64_t>(num)) mismatches++;
		uint32_t num32 = static_cast<uint32_t>(num);
		if (IntSqrt(num32) != cpp_reference::IntSqrtImplementation<uint32_t>(num32)) mismatches++;
	};

	/* Squares and the rounding boundaries around them. */
	for (uint64_t r = 0; r <= 0x10000; r++) {
		check(r * r);
		check(r * r + r);
		check(r * r + r + 1);
	}

	/* Powers of two and their neighbours, up to the maximum values. */
	for (uint shift = 0; shift < 64; shift++) {
		check((1ULL << shift) - 1);
		check(1ULL << shift);
		check((1ULL << shift) + 1);
	}
	check(std::numeric_limits<uint64_t>::max());

	/* A deterministic pseudo-random sample (xorshift64) of the whole range. */
	uint64_t state = 0x9E3779B97F4A7C15ULL;
	for (uint i = 0; i < 200000; i++) {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		check(state);
	}

	CHECK(mismatches == 0);
}


TEST_CASE("ClampTo")
{
	CHECK(0 == ClampTo<uint8_t>(std::numeric_limits<int64_t>::lowest()));
	CHECK(0 == ClampTo<uint8_t>(-1));
	CHECK(0 == ClampTo<uint8_t>(0));
	CHECK(1 == ClampTo<uint8_t>(1));

	CHECK(255 == ClampTo<uint8_t>(std::numeric_limits<uint64_t>::max()));
	CHECK(255 == ClampTo<uint8_t>(256));
	CHECK(255 == ClampTo<uint8_t>(255));
	CHECK(254 == ClampTo<uint8_t>(254));

	CHECK(-128 == ClampTo<int8_t>(std::numeric_limits<int64_t>::lowest()));
	CHECK(-128 == ClampTo<int8_t>(-129));
	CHECK(-128 == ClampTo<int8_t>(-128));
	CHECK(-127 == ClampTo<int8_t>(-127));

	CHECK(127 == ClampTo<int8_t>(std::numeric_limits<uint64_t>::max()));
	CHECK(127 == ClampTo<int8_t>(128));
	CHECK(127 == ClampTo<int8_t>(127));
	CHECK(126 == ClampTo<int8_t>(126));

	CHECK(126 == ClampTo<int64_t>(static_cast<uint8_t>(126)));
	CHECK(126 == ClampTo<uint64_t>(static_cast<int8_t>(126)));
	CHECK(0 == ClampTo<uint64_t>(static_cast<int8_t>(-126)));
	CHECK(0 == ClampTo<uint8_t>(static_cast<int8_t>(-126)));

	/* The realm around 64 bits types is tricky as there is not one type/method that works for all. */

	/* lowest/max uint64_t does not get clamped when clamping to uint64_t. */
	CHECK(std::numeric_limits<uint64_t>::lowest() == ClampTo<uint64_t>(std::numeric_limits<uint64_t>::lowest()));
	CHECK(std::numeric_limits<uint64_t>::max() == ClampTo<uint64_t>(std::numeric_limits<uint64_t>::max()));

	/* negative int64_t get clamped to 0. */
	CHECK(0 == ClampTo<uint64_t>(std::numeric_limits<int64_t>::lowest()));
	CHECK(0 == ClampTo<uint64_t>(int64_t(-1)));
	/* positive int64_t remain the same. */
	CHECK(1 == ClampTo<uint64_t>(int64_t(1)));
	CHECK(static_cast<uint64_t>(std::numeric_limits<int64_t>::max()) == ClampTo<uint64_t>(std::numeric_limits<int64_t>::max()));

	/* max uint64_t gets clamped to max int64_t. */
	CHECK(std::numeric_limits<int64_t>::max() == ClampTo<int64_t>(std::numeric_limits<uint64_t>::max()));
}


TEST_CASE("SoftClamp")
{
	/* Special behaviour of soft clamp returning the average of min/max when min is higher than max. */
	CHECK(1250 == SoftClamp(0, 1500, 1000));
	int million = 1000 * 1000;
	CHECK(1250 * million == SoftClamp(0, 1500 * million, 1000 * million));
	CHECK(0 == SoftClamp(0, 1500 * million, -1500 * million));
}

TEST_CASE("SaturatingAdd")
{
	CHECK(SaturatingAdd<uint8_t>(2, 3) == 5);
	CHECK(SaturatingAdd<uint8_t>(200, 200) == 255);
	CHECK(SaturatingAdd<uint8_t>(255, 255) == 255);
	CHECK(SaturatingAdd<uint8_t>(1, 255) == 255);
	CHECK(SaturatingAdd<uint8_t>(255, 1) == 255);
	CHECK(SaturatingAdd<uint8_t>(0, 254) == 254);
}

TEST_CASE("GetBase10DigitsRequired")
{
	CHECK(GetBase10DigitsRequired<uint32_t>(0) == 1);
	CHECK(GetBase10DigitsRequired<uint32_t>(1) == 1);
	CHECK(GetBase10DigitsRequired<uint32_t>(9) == 1);
	CHECK(GetBase10DigitsRequired<uint32_t>(10) == 2);
	CHECK(GetBase10DigitsRequired<uint32_t>(99) == 2);
	CHECK(GetBase10DigitsRequired<uint32_t>(100) == 3);
	CHECK(GetBase10DigitsRequired<uint32_t>(999) == 3);
	CHECK(GetBase10DigitsRequired<uint32_t>(1000) == 4);
	CHECK(GetBase10DigitsRequired<uint32_t>(9999) == 4);
	CHECK(GetBase10DigitsRequired<uint32_t>(10000) == 5);
	CHECK(GetBase10DigitsRequired<uint32_t>(99999) == 5);
	CHECK(GetBase10DigitsRequired<uint32_t>(100000) == 6);
	CHECK(GetBase10DigitsRequired<uint32_t>(999999) == 6);
	CHECK(GetBase10DigitsRequired<uint32_t>(1000000) == 7);
	CHECK(GetBase10DigitsRequired<uint32_t>(9999999) == 7);
	CHECK(GetBase10DigitsRequired<uint32_t>(10000000) == 8);
	CHECK(GetBase10DigitsRequired<uint32_t>(99999999) == 8);
	CHECK(GetBase10DigitsRequired<uint32_t>(100000000) == 9);
	CHECK(GetBase10DigitsRequired<uint32_t>(999999999) == 9);
	CHECK(GetBase10DigitsRequired<uint32_t>(1000000000) == 10);
	CHECK(GetBase10DigitsRequired<uint32_t>(UINT32_MAX) == 10);
	CHECK(GetBase10DigitsRequired<uint64_t>(9999999999ULL) == 10);
	CHECK(GetBase10DigitsRequired<uint64_t>(10000000000ULL) == 11);
	CHECK(GetBase10DigitsRequired<uint64_t>(99999999999ULL) == 11);
	CHECK(GetBase10DigitsRequired<uint64_t>(100000000000ULL) == 12);
	CHECK(GetBase10DigitsRequired<uint64_t>(999999999999ULL) == 12);
	CHECK(GetBase10DigitsRequired<uint64_t>(1000000000000000000ULL) == 19);
	CHECK(GetBase10DigitsRequired<uint64_t>(9999999999999999999ULL) == 19);
	CHECK(GetBase10DigitsRequired<uint64_t>(10000000000000000000ULL) == 20);
	CHECK(GetBase10DigitsRequired<uint64_t>(UINT64_MAX) == 20);
}
