/*
 * This file is part of OpenTTD.
 * OpenTTD is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 2.
 * OpenTTD is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 * See the GNU General Public License for more details. You should have received a copy of the GNU General Public License along with OpenTTD. If not, see <https://www.gnu.org/licenses/old-licenses/gpl-2.0>.
 */

/**
 * @file bmp.cpp Read support for bmps.
 * The decoding is implemented in Rust: openttd_core::bmp (see rust/PORTED.md).
 */

#include "stdafx.h"
#include "random_access_file_type.h"
#include "bmp.h"

#include "openttd_rs_bridge/lib.h"

#include "safeguards.h"

/**
 * Read a whole file into memory.
 * @param file The file to read.
 * @return The contents of the file, from its start to its end.
 */
static std::vector<uint8_t> ReadFileContents(RandomAccessFile &file)
{
	std::vector<uint8_t> contents(file.GetEndPos() - file.GetStartPos());
	file.SeekTo(file.GetStartPos(), SEEK_SET);
	file.ReadBlock(contents.data(), contents.size());
	return contents;
}

/**
 * Reads bitmap headers, and palette (if any).
 * @param file The contents of the file.
 * @param[out] info The metadata; \c offset is relative to the start of \a file.
 * @param[out] data The palette is read into this.
 * @return \c true iff the file is a BMP in a supported format.
 */
bool BmpReadHeader(std::span<const uint8_t> file, BmpInfo &info, BmpData &data)
{
	info = {};

	ottd_rs::BmpInfo rs_info{};
	rust::Vec<ottd_rs::BmpColour> palette;
	if (!ottd_rs::bmp_read_header(rust::Slice<const uint8_t>(file.data(), file.size()), rs_info, palette)) return false;

	info.offset = rs_info.offset;
	info.width = rs_info.width;
	info.height = rs_info.height;
	info.os2_bmp = rs_info.os2_bmp;
	info.bpp = rs_info.bpp;
	info.compression = rs_info.compression;
	info.palette_size = rs_info.palette_size;

	data.palette.clear();
	for (const ottd_rs::BmpColour &rs_colour : palette) {
		Colour &colour = data.palette.emplace_back();
		colour.r = rs_colour.r;
		colour.g = rs_colour.g;
		colour.b = rs_colour.b;
	}
	return true;
}

/**
 * Reads the bitmap. 1 bpp and 4 bpp bitmaps are converted to 8 bpp bitmaps.
 * @param file The contents of the file.
 * @param info The metadata read by BmpReadHeader; \c offset is relative to the start of \a file.
 * @param[out] data The bitmap is read into this.
 * @return \c true iff the bitmap could be read without problems.
 */
bool BmpReadBitmap(std::span<const uint8_t> file, const BmpInfo &info, BmpData &data)
{
	data.bitmap.resize(static_cast<size_t>(info.width) * info.height * ((info.bpp == 24) ? 3 : 1));

	ottd_rs::BmpInfo rs_info{info.offset, info.width, info.height, info.os2_bmp, info.bpp, info.compression, info.palette_size};
	return ottd_rs::bmp_read_bitmap(rust::Slice<const uint8_t>(file.data(), file.size()), rs_info,
			rust::Slice<uint8_t>(data.bitmap.data(), data.bitmap.size()));
}

/**
 * Reads bitmap headers, and palette (if any).
 * @param file The file to read.
 * @param[out] info The metadata.
 * @param[out] data The palette is read into this.
 * @return \c true iff the file is a BMP in a supported format.
 */
bool BmpReadHeader(RandomAccessFile &file, BmpInfo &info, BmpData &data)
{
	if (!BmpReadHeader(ReadFileContents(file), info, data)) return false;
	info.offset += file.GetStartPos();
	return true;
}

/**
 * Reads the bitmap. 1 bpp and 4 bpp bitmaps are converted to 8 bpp bitmaps.
 * @param file The file to read.
 * @param info The metadata read by BmpReadHeader.
 * @param[out] data The bitmap is read into this.
 * @return \c true iff the bitmap could be read without problems.
 */
bool BmpReadBitmap(RandomAccessFile &file, BmpInfo &info, BmpData &data)
{
	BmpInfo relative_info = info;
	relative_info.offset -= file.GetStartPos();
	return BmpReadBitmap(ReadFileContents(file), relative_info, data);
}
