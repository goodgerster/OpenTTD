/*
 * This file is part of OpenTTD.
 * OpenTTD is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 2.
 * OpenTTD is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 * See the GNU General Public License for more details. You should have received a copy of the GNU General Public License along with OpenTTD. If not, see <https://www.gnu.org/licenses/old-licenses/gpl-2.0>.
 */

/** @file bmp.cpp Test reading BMP files (implemented in Rust, see rust/PORTED.md). */

#include "../stdafx.h"

#include "../3rdparty/catch2/catch.hpp"

#include "../bmp.h"
#include "../core/bitmath_func.hpp"

#include "../safeguards.h"

/** Original C++ implementations of functions ported to Rust, used to check the ports against. */
namespace cpp_reference {

/**
 * In-memory stand-in for RandomAccessFile on a standalone file: reads past the
 * end yield zeros without advancing, and skips may move past the end.
 */
class SpanFile {
	std::span<const uint8_t> data; ///< Contents of the file.
	size_t pos = 0; ///< Current position.

public:
	/**
	 * Create a file with the given contents.
	 * @param data Contents of the file.
	 */
	explicit SpanFile(std::span<const uint8_t> data) : data(data) {}

	size_t GetPos() const { return this->pos; }
	size_t GetStartPos() const { return 0; }
	bool AtEndOfFile() const { return this->pos >= this->data.size(); }
	void SeekTo(size_t pos, int) { this->pos = pos; }
	void SkipBytes(size_t n) { this->pos += n; }
	uint8_t ReadByte() { return this->pos < this->data.size() ? this->data[this->pos++] : 0; }

	uint16_t ReadWord()
	{
		uint8_t b = this->ReadByte();
		return (this->ReadByte() << 8) | b;
	}

	uint32_t ReadDword()
	{
		uint b = this->ReadWord();
		return (this->ReadWord() << 16) | b;
	}
};

/*
 * The original implementation from src/bmp.cpp, unchanged except that the file
 * type is a template parameter, so that it can read from a SpanFile.
 */

/**
 * Reads a 1 bpp uncompressed bitmap. The bitmap is converted to a 8 bpp bitmap.
 * @copydoc BmpReadBitmap
 */
template <typename TFile>
static inline bool BmpRead1(TFile &file, BmpInfo &info, BmpData &data)
{
	uint8_t pad = GB(4 - info.width / 8, 0, 2);
	for (uint y = info.height; y > 0; y--) {
		uint x = 0;
		uint8_t *pixel_row = &data.bitmap[(y - 1) * static_cast<size_t>(info.width)];
		while (x < info.width) {
			if (file.AtEndOfFile()) return false; // the file is shorter than expected
			uint8_t b = file.ReadByte();
			for (uint i = 8; i > 0; i--) {
				if (x < info.width) *pixel_row++ = GB(b, i - 1, 1);
				x++;
			}
		}
		/* Padding for 32 bit align */
		file.SkipBytes(pad);
	}
	return true;
}

/**
 * Reads a 4 bpp uncompressed bitmap. The bitmap is converted to a 8 bpp bitmap.
 * @copydoc BmpReadBitmap
 */
template <typename TFile>
static inline bool BmpRead4(TFile &file, BmpInfo &info, BmpData &data)
{
	uint8_t pad = GB(4 - info.width / 2, 0, 2);
	for (uint y = info.height; y > 0; y--) {
		uint x = 0;
		uint8_t *pixel_row = &data.bitmap[(y - 1) * static_cast<size_t>(info.width)];
		while (x < info.width) {
			if (file.AtEndOfFile()) return false;  // the file is shorter than expected
			uint8_t b = file.ReadByte();
			*pixel_row++ = GB(b, 4, 4);
			x++;
			if (x < info.width) {
				*pixel_row++ = GB(b, 0, 4);
				x++;
			}
		}
		/* Padding for 32 bit align */
		file.SkipBytes(pad);
	}
	return true;
}

/**
 * Reads a 4-bit RLE compressed bitmap. The bitmap is converted to a 8 bpp bitmap.
 * @copydoc BmpReadBitmap
 */
template <typename TFile>
static inline bool BmpRead4Rle(TFile &file, BmpInfo &info, BmpData &data)
{
	uint x = 0;
	uint y = info.height - 1;
	uint8_t *pixel = &data.bitmap[y * static_cast<size_t>(info.width)];
	while (y != 0 || x < info.width) {
		if (file.AtEndOfFile()) return false; // the file is shorter than expected

		uint8_t n = file.ReadByte();
		uint8_t c = file.ReadByte();
		if (n == 0) {
			switch (c) {
				case 0: // end of line
					x = 0;
					if (y == 0) return false;
					pixel = &data.bitmap[--y * static_cast<size_t>(info.width)];
					break;

				case 1: // end of bitmap
					return true;

				case 2: { // delta
					if (file.AtEndOfFile()) return false;
					uint8_t dx = file.ReadByte();
					uint8_t dy = file.ReadByte();

					/* Check for over- and underflow. */
					if (x + dx >= info.width || x + dx < x || dy > y) return false;

					x += dx;
					y -= dy;
					pixel = &data.bitmap[y * info.width + x];
					break;
				}

				default: { // uncompressed
					uint i = 0;
					while (i++ < c) {
						if (file.AtEndOfFile() || x >= info.width) return false;
						uint8_t b = file.ReadByte();
						*pixel++ = GB(b, 4, 4);
						x++;
						if (i++ < c) {
							if (x >= info.width) return false;
							*pixel++ = GB(b, 0, 4);
							x++;
						}
					}
					/* Padding for 16 bit align */
					file.SkipBytes(((c + 1) / 2) % 2);
					break;
				}
			}
		} else {
			/* Apparently it is common to encounter BMPs where the count of
			 * pixels to be written is higher than the remaining line width.
			 * Ignore the superfluous pixels instead of reporting an error. */
			uint i = 0;
			while (x < info.width && i++ < n) {
				*pixel++ = GB(c, 4, 4);
				x++;
				if (x < info.width && i++ < n) {
					*pixel++ = GB(c, 0, 4);
					x++;
				}
			}
		}
	}
	return true;
}

/** Reads a 8 bpp bitmap. @copydoc BmpReadBitmap */
template <typename TFile>
static inline bool BmpRead8(TFile &file, BmpInfo &info, BmpData &data)
{
	uint8_t pad = GB(4 - info.width, 0, 2);
	for (uint y = info.height; y > 0; y--) {
		if (file.AtEndOfFile()) return false; // the file is shorter than expected
		uint8_t *pixel = &data.bitmap[(y - 1) * static_cast<size_t>(info.width)];
		for (uint i = 0; i < info.width; i++) *pixel++ = file.ReadByte();
		/* Padding for 32 bit align */
		file.SkipBytes(pad);
	}
	return true;
}

/** Reads a 8-bit RLE compressed bpp bitmap. @copydoc BmpReadBitmap */
template <typename TFile>
static inline bool BmpRead8Rle(TFile &file, BmpInfo &info, BmpData &data)
{
	uint x = 0;
	uint y = info.height - 1;
	uint8_t *pixel = &data.bitmap[y * static_cast<size_t>(info.width)];
	while (y != 0 || x < info.width) {
		if (file.AtEndOfFile()) return false; // the file is shorter than expected

		uint8_t n = file.ReadByte();
		uint8_t c = file.ReadByte();
		if (n == 0) {
			switch (c) {
				case 0: // end of line
					x = 0;
					if (y == 0) return false;
					pixel = &data.bitmap[--y * static_cast<size_t>(info.width)];
					break;

				case 1: // end of bitmap
					return true;

				case 2: { // delta
					if (file.AtEndOfFile()) return false;
					uint8_t dx = file.ReadByte();
					uint8_t dy = file.ReadByte();

					/* Check for over- and underflow. */
					if (x + dx >= info.width || x + dx < x || dy > y) return false;

					x += dx;
					y -= dy;
					pixel = &data.bitmap[y * static_cast<size_t>(info.width) + x];
					break;
				}

				default: { // uncompressed
					for (uint i = 0; i < c; i++) {
						if (file.AtEndOfFile() || x >= info.width) return false;
						*pixel++ = file.ReadByte();
						x++;
					}
					/* Padding for 16 bit align */
					file.SkipBytes(c % 2);
					break;
				}
			}
		} else {
			/* Apparently it is common to encounter BMPs where the count of
			 * pixels to be written is higher than the remaining line width.
			 * Ignore the superfluous pixels instead of reporting an error. */
			for (uint i = 0; x < info.width && i < n; i++) {
				*pixel++ = c;
				x++;
			}
		}
	}
	return true;
}

/** Reads a 24 bpp uncompressed bitmap. @copydoc BmpReadBitmap */
template <typename TFile>
static inline bool BmpRead24(TFile &file, BmpInfo &info, BmpData &data)
{
	uint8_t pad = GB(4 - info.width * 3, 0, 2);
	for (uint y = info.height; y > 0; --y) {
		uint8_t *pixel_row = &data.bitmap[(y - 1) * static_cast<size_t>(info.width) * 3];
		for (uint x = 0; x < info.width; ++x) {
			if (file.AtEndOfFile()) return false; // the file is shorter than expected
			*(pixel_row + 2) = file.ReadByte(); // green
			*(pixel_row + 1) = file.ReadByte(); // blue
			*pixel_row       = file.ReadByte(); // red
			pixel_row += 3;
		}
		/* Padding for 32 bit align */
		file.SkipBytes(pad);
	}
	return true;
}

/** Reads bitmap headers, and palette (if any). @copydoc BmpReadBitmap */
template <typename TFile>
bool BmpReadHeader(TFile &file, BmpInfo &info, BmpData &data)
{
	info = {};

	/* Reading BMP header */
	if (file.ReadWord() != 0x4D42) return false; // signature should be 'BM'
	file.SkipBytes(8); // skip file size and reserved
	info.offset = file.ReadDword() + file.GetStartPos();

	/* Reading info header */
	uint32_t header_size = file.ReadDword();
	if (header_size < 12) return false; // info header should be at least 12 bytes long

	info.os2_bmp = (header_size == 12); // OS/2 1.x or windows 2.x info header is 12 bytes long

	if (info.os2_bmp) {
		info.width = file.ReadWord();
		info.height = file.ReadWord();
		header_size -= 8;
	} else {
		info.width = file.ReadDword();
		info.height = file.ReadDword();
		header_size -= 12;
	}

	if (file.ReadWord() != 1) return false; // BMP can have only 1 plane

	info.bpp = file.ReadWord();
	if (info.bpp != 1 && info.bpp != 4 && info.bpp != 8 && info.bpp != 24) {
		/* Only 1 bpp, 4 bpp, 8bpp and 24 bpp bitmaps are supported */
		return false;
	}

	/* Reads compression method if available in info header*/
	if ((header_size -= 4) >= 4) {
		info.compression = file.ReadDword();
		header_size -= 4;
	}

	/* Only 4-bit and 8-bit rle compression is supported */
	if (info.compression > 2 || (info.compression > 0 && !(info.bpp == 4 || info.bpp == 8))) return false;

	if (info.bpp <= 8) {
		/* Reads number of colours if available in info header */
		if (header_size >= 16) {
			file.SkipBytes(12);                  // skip image size and resolution
			info.palette_size = file.ReadDword(); // number of colours in palette
			file.SkipBytes(header_size - 16);    // skip the end of info header
		}

		uint maximum_palette_size = 1U << info.bpp;
		if (info.palette_size == 0) info.palette_size = maximum_palette_size;

		/* More palette colours than palette indices is not supported. */
		if (info.palette_size > maximum_palette_size) return false;

		data.palette.resize(info.palette_size);

		for (auto &colour : data.palette) {
			colour.b = file.ReadByte();
			colour.g = file.ReadByte();
			colour.r = file.ReadByte();
			if (!info.os2_bmp) file.SkipBytes(1); // unused
		}
	}

	return file.GetPos() <= info.offset;
}

/**
 * Reads the bitmap
 * 1 bpp and 4 bpp bitmaps are converted to 8 bpp bitmaps
 * @param file The file to read.
 * @param info The already read medata.
 * @param data The buffer to read the image into.
 * @return \c true iff the file could be read without problems.
 */
template <typename TFile>
bool BmpReadBitmap(TFile &file, BmpInfo &info, BmpData &data)
{
	data.bitmap.resize(static_cast<size_t>(info.width) * info.height * ((info.bpp == 24) ? 3 : 1));

	/* Load image */
	file.SeekTo(info.offset, SEEK_SET);
	switch (info.compression) {
		case 0: // no compression
			switch (info.bpp) {
				case 1: return BmpRead1(file, info, data);
				case 4: return BmpRead4(file, info, data);
				case 8: return BmpRead8(file, info, data);
				case 24: return BmpRead24(file, info, data);
				default: NOT_REACHED();
			}
			break;

		case 1: return BmpRead8Rle(file, info, data); // 8-bit RLE compression
		case 2: return BmpRead4Rle(file, info, data); // 4-bit RLE compression
		default: NOT_REACHED();
	}
}

} // namespace cpp_reference

/**
 * Build a BMP file with a 40-byte info header.
 * @param width Width in pixels.
 * @param height Height in pixels.
 * @param bpp Bits per pixel.
 * @param compression Compression method.
 * @param palette_size Number of palette entries; 0 means the maximum for \a bpp, as in the format.
 * @param pixels The bitmap data.
 * @return The file contents.
 */
static std::vector<uint8_t> MakeBmp(uint32_t width, uint32_t height, uint16_t bpp, uint32_t compression, uint32_t palette_size, std::span<const uint8_t> pixels)
{
	uint32_t palette_entries = (bpp > 8) ? 0 : (palette_size != 0 ? palette_size : (1U << bpp));
	uint32_t offset = 14 + 40 + 4 * palette_entries;

	std::vector<uint8_t> file;
	auto put16 = [&file](uint16_t v) { file.push_back(GB(v, 0, 8)); file.push_back(GB(v, 8, 8)); };
	auto put32 = [&put16](uint32_t v) { put16(GB(v, 0, 16)); put16(GB(v, 16, 16)); };

	put16(0x4D42);
	put32(offset + static_cast<uint32_t>(pixels.size()));
	put32(0);
	put32(offset);
	put32(40);
	put32(width);
	put32(height);
	put16(1);
	put16(bpp);
	put32(compression);
	put32(0);
	put32(0);
	put32(0);
	put32(palette_size);
	put32(0);
	for (uint32_t i = 0; i < palette_entries; i++) put32(i * 0x010101);
	file.insert(file.end(), pixels.begin(), pixels.end());
	return file;
}

TEST_CASE("BMP - reads an 8 bpp image bottom up")
{
	const uint8_t pixels[] = {1, 2, 3, 0, 4, 5, 6, 0};
	std::vector<uint8_t> file = MakeBmp(3, 2, 8, 0, 8, pixels);

	BmpInfo info{};
	BmpData data{};
	REQUIRE(BmpReadHeader(file, info, data));
	CHECK(info.width == 3);
	CHECK(info.height == 2);
	CHECK(info.bpp == 8);
	CHECK(data.palette.size() == 8);
	CHECK(data.palette[7].r == 7);

	REQUIRE(BmpReadBitmap(file, info, data));
	CHECK(data.bitmap == std::vector<uint8_t>{4, 5, 6, 1, 2, 3});
}

TEST_CASE("BMP - rejects files that are not BMPs")
{
	const uint8_t not_a_bmp[] = {'G', 'I', 'F', '8', '9', 'a'};
	BmpInfo info{};
	BmpData data{};
	CHECK_FALSE(BmpReadHeader(not_a_bmp, info, data));
	CHECK_FALSE(BmpReadHeader(std::span<const uint8_t>{}, info, data));
}

/** Deterministic pseudo-random numbers (xorshift64). */
struct XorShift {
	uint64_t state = 0x9E3779B97F4A7C15ULL; ///< Current state; never 0.

	/** @return The next number. */
	uint64_t Next()
	{
		this->state ^= this->state << 13;
		this->state ^= this->state >> 7;
		this->state ^= this->state << 17;
		return this->state;
	}

	/**
	 * @param n Upper bound, exclusive; must not be 0.
	 * @return A number below \a n.
	 */
	uint32_t Below(uint32_t n) { return static_cast<uint32_t>(this->Next() % n); }
};

/**
 * Generate RLE data: a random mix of runs, literal pixels, deltas, ends of line
 * and an occasional end of bitmap, valid or not.
 * @param rng Random number generator.
 * @return The RLE data.
 */
static std::vector<uint8_t> MakeRle(XorShift &rng)
{
	std::vector<uint8_t> data;
	uint ops = rng.Below(40);
	for (uint op = 0; op < ops; op++) {
		switch (rng.Below(6)) {
			case 0:
			case 1: // run
				data.push_back(1 + rng.Below(12));
				data.push_back(rng.Below(256));
				break;

			case 2: { // literal pixels, padded
				uint8_t count = 3 + rng.Below(10);
				data.push_back(0);
				data.push_back(count);
				for (uint i = 0; i < count; i++) data.push_back(rng.Below(256));
				if (rng.Below(2) == 0) data.push_back(0);
				break;
			}

			case 3: // delta
				data.push_back(0);
				data.push_back(2);
				data.push_back(rng.Below(6));
				data.push_back(rng.Below(3));
				break;

			case 4: // end of line
				data.push_back(0);
				data.push_back(0);
				break;

			case 5: // end of bitmap, sometimes
				if (rng.Below(4) == 0) {
					data.push_back(0);
					data.push_back(1);
				}
				break;
		}
	}
	return data;
}

/**
 * Read a file with both the reference implementation and the ported one.
 * @param file The file.
 * @return Whether both gave the same results.
 */
static bool MatchesReference(std::span<const uint8_t> file)
{
	BmpInfo ref_info{};
	BmpData ref_data{};
	cpp_reference::SpanFile ref_file(file);
	bool ref_ok = cpp_reference::BmpReadHeader(ref_file, ref_info, ref_data);

	BmpInfo info{};
	BmpData data{};
	bool ok = BmpReadHeader(file, info, data);

	if (ok != ref_ok) return false;
	if (!ok) return true;

	if (info.offset != ref_info.offset || info.width != ref_info.width || info.height != ref_info.height ||
			info.os2_bmp != ref_info.os2_bmp || info.bpp != ref_info.bpp || info.compression != ref_info.compression ||
			info.palette_size != ref_info.palette_size || data.palette.size() != ref_data.palette.size()) {
		return false;
	}
	for (size_t i = 0; i < data.palette.size(); i++) {
		if (data.palette[i].data != ref_data.palette[i].data) return false;
	}

	/* Like heightmap.cpp, only read the bitmap of images with valid dimensions. */
	if (info.width == 0 || info.height == 0 || static_cast<uint64_t>(info.width) * info.height > 4096) return true;

	ref_ok = cpp_reference::BmpReadBitmap(ref_file, ref_info, ref_data);
	ok = BmpReadBitmap(file, info, data);
	return ok == ref_ok && (!ok || data.bitmap == ref_data.bitmap);
}

TEST_CASE("BMP - matches C++ reference implementation")
{
	struct Format {
		uint16_t bpp;
		uint32_t compression;
	};
	static const Format formats[] = {{1, 0}, {4, 0}, {8, 0}, {24, 0}, {8, 1}, {4, 2}};

	XorShift rng;
	uint64_t cases = 0;
	uint64_t mismatches = 0;
	auto check = [&](std::span<const uint8_t> file) {
		cases++;
		if (!MatchesReference(file)) mismatches++;
	};

	for (const Format &format : formats) {
		for (uint n = 0; n < 400; n++) {
			uint32_t width = 1 + rng.Below(24);
			uint32_t height = 1 + rng.Below(24);
			uint32_t palette_size = (format.bpp > 8) ? 0 : rng.Below((1U << format.bpp) + 1);

			std::vector<uint8_t> pixels;
			if (format.compression == 0) {
				/* Enough data for any padding, plus or minus a little. */
				size_t size = (static_cast<size_t>(width) * format.bpp / 8 + 4) * height;
				size = size + rng.Below(8) - std::min<size_t>(size, rng.Below(8));
				for (size_t i = 0; i < size; i++) pixels.push_back(rng.Below(256));
			} else {
				pixels = MakeRle(rng);
			}
			std::vector<uint8_t> file = MakeBmp(width, height, format.bpp, format.compression, palette_size, pixels);
			check(file);

			/* Truncated. */
			check(std::span<const uint8_t>(file).first(rng.Below(static_cast<uint32_t>(file.size()) + 1)));

			/* A few corrupted bytes, possibly in the header. */
			std::vector<uint8_t> corrupted = file;
			for (uint i = 1 + rng.Below(3); i > 0; i--) {
				corrupted[rng.Below(static_cast<uint32_t>(corrupted.size()))] = static_cast<uint8_t>(rng.Next());
			}
			check(corrupted);

			/* An OS/2 header instead. */
			std::vector<uint8_t> os2 = file;
			os2[14] = 12;
			check(os2);
		}
	}

	CHECK(cases == 6 * 400 * 4);
	CHECK(mismatches == 0);
}
