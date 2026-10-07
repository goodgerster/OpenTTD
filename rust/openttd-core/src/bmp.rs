//! BMP decoding for heightmaps, ported from `src/bmp.cpp`.
//!
//! Supports uncompressed 1, 4, 8 and 24 bits per pixel, and 4- and 8-bit RLE
//! compression. 1- and 4-bit images are expanded to one byte per pixel; 24-bit
//! images become RGB triplets. Rows are stored top to bottom.
//!
//! The decoder works on the bytes of the whole file. Reading past the end
//! behaves like OpenTTD's `RandomAccessFile` on a standalone file: reads yield
//! zeros without advancing, and skips may move past the end.

/// Header information of a BMP file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BmpInfo {
    /// Offset of the bitmap data from the start of the file.
    pub offset: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Whether the file has an OS/2 1.x (or Windows 2.x) header.
    pub os2_bmp: bool,
    /// Bits per pixel: 1, 4, 8 or 24.
    pub bpp: u16,
    /// Compression method: 0 = none, 1 = 8-bit RLE, 2 = 4-bit RLE.
    pub compression: u32,
    /// Number of colours in the palette (0 for 24-bit images).
    pub palette_size: u32,
}

/// A palette entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaletteColour {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// The header and palette of a BMP file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BmpHeader {
    /// Header information.
    pub info: BmpInfo,
    /// The palette; empty for 24-bit images.
    pub palette: Vec<PaletteColour>,
}

/// Sequential little-endian reader with `RandomAccessFile` end-of-file semantics.
struct Reader<'a> {
    data: &'a [u8],
    pos: u64,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Reads a byte, or yields 0 without advancing at or past the end.
    fn byte(&mut self) -> u8 {
        match usize::try_from(self.pos)
            .ok()
            .and_then(|pos| self.data.get(pos))
        {
            Some(&b) => {
                self.pos += 1;
                b
            }
            None => 0,
        }
    }

    fn word(&mut self) -> u16 {
        let low = self.byte();
        u16::from(low) | u16::from(self.byte()) << 8
    }

    fn dword(&mut self) -> u32 {
        let low = self.word();
        u32::from(low) | u32::from(self.word()) << 16
    }

    /// Moves forward; may move past the end.
    fn skip(&mut self, n: u64) {
        self.pos += n;
    }

    fn seek(&mut self, pos: u64) {
        self.pos = pos;
    }

    fn at_end(&self) -> bool {
        self.pos >= self.data.len() as u64
    }
}

/// Reads the header and palette of a BMP file.
///
/// Returns `None` if the file is not a BMP in a supported format.
pub fn read_header(file: &[u8]) -> Option<BmpHeader> {
    let mut f = Reader::new(file);
    let mut info = BmpInfo::default();

    // File header
    if f.word() != 0x4D42 {
        return None; // signature should be "BM"
    }
    f.skip(8); // file size and reserved
    info.offset = u64::from(f.dword());

    // Info header
    let mut header_size = f.dword();
    if header_size < 12 {
        return None;
    }
    info.os2_bmp = header_size == 12; // OS/2 1.x or Windows 2.x info header
    if info.os2_bmp {
        info.width = u32::from(f.word());
        info.height = u32::from(f.word());
        header_size -= 8;
    } else {
        info.width = f.dword();
        info.height = f.dword();
        header_size -= 12;
    }

    if f.word() != 1 {
        return None; // only one plane is allowed
    }
    info.bpp = f.word();
    if !matches!(info.bpp, 1 | 4 | 8 | 24) {
        return None;
    }

    // The compression method, if the header has one. Header sizes of 13 to 15
    // wrap around here, as the unsigned subtraction in the original does.
    header_size = header_size.wrapping_sub(4);
    if header_size >= 4 {
        info.compression = f.dword();
        header_size -= 4;
    }
    // Only 8-bit and 4-bit RLE compression are supported.
    if info.compression > 2 || (info.compression > 0 && !matches!(info.bpp, 4 | 8)) {
        return None;
    }

    let mut palette = Vec::new();
    if info.bpp <= 8 {
        // The number of colours, if the header has it.
        if header_size >= 16 {
            f.skip(12); // image size and resolution
            info.palette_size = f.dword();
            f.skip(u64::from(header_size - 16)); // rest of the info header
        }
        let maximum_palette_size = 1u32 << info.bpp;
        if info.palette_size == 0 {
            info.palette_size = maximum_palette_size;
        }
        if info.palette_size > maximum_palette_size {
            return None;
        }
        palette = (0..info.palette_size)
            .map(|_| {
                let b = f.byte();
                let g = f.byte();
                let r = f.byte();
                if !info.os2_bmp {
                    f.skip(1); // unused
                }
                PaletteColour { r, g, b }
            })
            .collect();
    }

    (f.pos <= info.offset).then_some(BmpHeader { info, palette })
}

/// Decodes the bitmap of a BMP file whose header was read by [`read_header`].
///
/// `bitmap` must hold `width * height` bytes (three times that for 24-bit
/// images); pixels that the file does not set are left as they are. Returns
/// `false` if the bitmap data is truncated or invalid.
pub fn read_bitmap(file: &[u8], info: &BmpInfo, bitmap: &mut [u8]) -> bool {
    let bytes_per_pixel = if info.bpp == 24 { 3 } else { 1 };
    let expected_len = (info.width as usize)
        .checked_mul(info.height as usize)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel));
    if expected_len != Some(bitmap.len()) {
        return false;
    }

    let mut f = Reader::new(file);
    f.seek(info.offset);
    match (info.compression, info.bpp) {
        (0, 1) => read_1bpp(&mut f, info, bitmap),
        (0, 4) => read_4bpp(&mut f, info, bitmap),
        (0, 8) => read_8bpp(&mut f, info, bitmap),
        (0, 24) => read_24bpp(&mut f, info, bitmap),
        (1, 8) => read_rle(&mut f, info, bitmap, Rle::EightBit),
        (2, 4) => read_rle(&mut f, info, bitmap, Rle::FourBit),
        // The original never gets here: its header check rejects these.
        _ => false,
    }
}

/// Row padding to a multiple of four bytes: `(4 - row_bytes) % 4`, computed
/// with unsigned wrapping like the original. Unlike the original, 1 and 4 bpp
/// rows count their last, partly used byte (see `rust/PORTED.md`).
fn padding(row_bytes: u32) -> u64 {
    u64::from(4u32.wrapping_sub(row_bytes) & 3)
}

/// Iterates over the start indices of the rows, bottom row first.
fn rows_bottom_up(info: &BmpInfo, row_len: usize) -> impl Iterator<Item = usize> {
    (0..info.height as usize).rev().map(move |y| y * row_len)
}

fn read_1bpp(f: &mut Reader, info: &BmpInfo, bitmap: &mut [u8]) -> bool {
    let width = info.width as usize;
    let pad = padding(info.width.div_ceil(8));
    for row in rows_bottom_up(info, width) {
        let mut x = 0;
        while x < width {
            if f.at_end() {
                return false;
            }
            let b = f.byte();
            for bit in (0..8).rev() {
                if x < width {
                    bitmap[row + x] = (b >> bit) & 1;
                }
                x += 1;
            }
        }
        f.skip(pad);
    }
    true
}

fn read_4bpp(f: &mut Reader, info: &BmpInfo, bitmap: &mut [u8]) -> bool {
    let width = info.width as usize;
    let pad = padding(info.width.div_ceil(2));
    for row in rows_bottom_up(info, width) {
        let mut x = 0;
        while x < width {
            if f.at_end() {
                return false;
            }
            let b = f.byte();
            bitmap[row + x] = b >> 4;
            x += 1;
            if x < width {
                bitmap[row + x] = b & 0xF;
                x += 1;
            }
        }
        f.skip(pad);
    }
    true
}

fn read_8bpp(f: &mut Reader, info: &BmpInfo, bitmap: &mut [u8]) -> bool {
    let width = info.width as usize;
    let pad = padding(info.width);
    for row in rows_bottom_up(info, width) {
        // Only checked once per row; the rest of a row past the end reads as zeros.
        if f.at_end() {
            return false;
        }
        for pixel in &mut bitmap[row..row + width] {
            *pixel = f.byte();
        }
        f.skip(pad);
    }
    true
}

fn read_24bpp(f: &mut Reader, info: &BmpInfo, bitmap: &mut [u8]) -> bool {
    let width = info.width as usize;
    let pad = padding(info.width.wrapping_mul(3));
    for row in rows_bottom_up(info, width * 3) {
        for pixel in bitmap[row..row + width * 3].chunks_exact_mut(3) {
            if f.at_end() {
                return false;
            }
            // The file stores blue, green, red.
            pixel[2] = f.byte();
            pixel[1] = f.byte();
            pixel[0] = f.byte();
        }
        f.skip(pad);
    }
    true
}

/// The two RLE variants.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rle {
    /// 8 bits per pixel (BI_RLE8).
    EightBit,
    /// 4 bits per pixel (BI_RLE4); each byte holds two pixels.
    FourBit,
}

/// Decodes RLE data, mirroring `BmpRead8Rle` and `BmpRead4Rle` in the original.
fn read_rle(f: &mut Reader, info: &BmpInfo, bitmap: &mut [u8], rle: Rle) -> bool {
    let width = info.width;
    // The original has undefined behaviour here; there is no row to start on.
    let Some(mut y) = info.height.checked_sub(1) else {
        return false;
    };
    let mut x: u32 = 0;
    let index = |x: u32, y: u32| y as usize * width as usize + x as usize;

    while y != 0 || x < width {
        if f.at_end() {
            return false; // the file is shorter than expected
        }
        let n = f.byte();
        let c = f.byte();
        if n == 0 {
            match c {
                0 => {
                    // end of line
                    x = 0;
                    if y == 0 {
                        return false;
                    }
                    y -= 1;
                }
                1 => return true, // end of bitmap
                2 => {
                    // delta
                    if f.at_end() {
                        return false;
                    }
                    let dx = u32::from(f.byte());
                    let dy = u32::from(f.byte());
                    let new_x = x.wrapping_add(dx);
                    if new_x >= width || new_x < x || dy > y {
                        return false;
                    }
                    x = new_x;
                    y -= dy;
                }
                _ => {
                    // c literal pixels
                    if !read_rle_literal(f, bitmap, rle, c, width, &mut x, index(0, y)) {
                        return false;
                    }
                }
            }
        } else {
            // A run of n pixels. Runs longer than the rest of the row are
            // common in the wild; the excess is ignored.
            let row = index(0, y);
            let mut i = 0;
            while x < width && i < n {
                bitmap[row + x as usize] = match rle {
                    Rle::EightBit => c,
                    Rle::FourBit if i % 2 == 0 => c >> 4,
                    Rle::FourBit => c & 0xF,
                };
                x += 1;
                i += 1;
            }
        }
    }
    true
}

/// Reads `count` literal pixels and the padding to an even number of bytes.
fn read_rle_literal(
    f: &mut Reader,
    bitmap: &mut [u8],
    rle: Rle,
    count: u8,
    width: u32,
    x: &mut u32,
    row: usize,
) -> bool {
    match rle {
        Rle::EightBit => {
            for _ in 0..count {
                if f.at_end() || *x >= width {
                    return false;
                }
                bitmap[row + *x as usize] = f.byte();
                *x += 1;
            }
            f.skip(u64::from(count % 2));
        }
        Rle::FourBit => {
            let mut i = 0;
            while i < count {
                if f.at_end() || *x >= width {
                    return false;
                }
                let b = f.byte();
                bitmap[row + *x as usize] = b >> 4;
                *x += 1;
                i += 1;
                if i < count {
                    if *x >= width {
                        return false;
                    }
                    bitmap[row + *x as usize] = b & 0xF;
                    *x += 1;
                    i += 1;
                }
            }
            // Bytes used: (count + 1) / 2, padded to an even number.
            f.skip(u64::from(count.div_ceil(2) % 2));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a BMP file with a 40-byte Windows info header.
    fn windows_bmp(
        width: u32,
        height: u32,
        bpp: u16,
        compression: u32,
        palette: &[[u8; 3]],
        pixels: &[u8],
    ) -> Vec<u8> {
        let offset = 14 + 40 + 4 * palette.len() as u32;
        let mut file = Vec::new();
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&(offset + pixels.len() as u32).to_le_bytes());
        file.extend_from_slice(&[0; 4]);
        file.extend_from_slice(&offset.to_le_bytes());
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(&width.to_le_bytes());
        file.extend_from_slice(&height.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&bpp.to_le_bytes());
        file.extend_from_slice(&compression.to_le_bytes());
        file.extend_from_slice(&[0; 12]); // image size and resolution
        file.extend_from_slice(&(palette.len() as u32).to_le_bytes());
        file.extend_from_slice(&[0; 4]); // important colours
        for [r, g, b] in palette {
            file.extend_from_slice(&[*b, *g, *r, 0]);
        }
        file.extend_from_slice(pixels);
        file
    }

    fn grey_palette(n: usize) -> Vec<[u8; 3]> {
        (0..n).map(|i| [i as u8; 3]).collect()
    }

    fn decode(file: &[u8]) -> Option<Vec<u8>> {
        let header = read_header(file)?;
        let info = &header.info;
        let len = info.width as usize * info.height as usize * if info.bpp == 24 { 3 } else { 1 };
        let mut bitmap = vec![0; len];
        read_bitmap(file, info, &mut bitmap).then_some(bitmap)
    }

    #[test]
    fn header_of_windows_8bpp_file() {
        let file = windows_bmp(3, 2, 8, 0, &grey_palette(4), &[0; 8]);
        let header = read_header(&file).unwrap();
        assert_eq!(
            header.info,
            BmpInfo {
                offset: 14 + 40 + 16,
                width: 3,
                height: 2,
                os2_bmp: false,
                bpp: 8,
                compression: 0,
                palette_size: 4,
            }
        );
        assert_eq!(header.palette[3], PaletteColour { r: 3, g: 3, b: 3 });
    }

    #[test]
    fn header_of_os2_file() {
        let mut file = Vec::new();
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&[0; 8]);
        file.extend_from_slice(&(14u32 + 12 + 6).to_le_bytes());
        file.extend_from_slice(&12u32.to_le_bytes());
        file.extend_from_slice(&5u16.to_le_bytes());
        file.extend_from_slice(&7u16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&[1, 2, 3, 4, 5, 6]); // two 3-byte palette entries (BGR)
        let header = read_header(&file).unwrap();
        assert!(header.info.os2_bmp);
        assert_eq!((header.info.width, header.info.height), (5, 7));
        assert_eq!(header.info.palette_size, 2);
        assert_eq!(
            header.palette,
            [
                PaletteColour { r: 3, g: 2, b: 1 },
                PaletteColour { r: 6, g: 5, b: 4 }
            ]
        );
    }

    #[test]
    fn unused_palette_size_means_full_palette() {
        let mut file = windows_bmp(1, 1, 4, 0, &[], &[0; 4]);
        file.splice(54..54, [0; 64]); // 16 palette entries
        file[10] = 54 + 64; // bitmap offset
        let header = read_header(&file).unwrap();
        assert_eq!(header.info.palette_size, 16);
        assert_eq!(header.palette.len(), 16);
    }

    #[test]
    fn unsupported_headers_are_rejected() {
        let valid = windows_bmp(1, 1, 8, 0, &grey_palette(1), &[0; 4]);
        assert!(read_header(&valid).is_some());

        let mut wrong_signature = valid.clone();
        wrong_signature[0] = b'X';
        assert!(read_header(&wrong_signature).is_none());

        let mut two_planes = valid.clone();
        two_planes[26] = 2;
        assert!(read_header(&two_planes).is_none());

        let mut bpp16 = valid.clone();
        bpp16[28] = 16;
        assert!(read_header(&bpp16).is_none());

        let mut rle_for_24bpp = windows_bmp(1, 1, 24, 1, &[], &[0; 4]);
        assert!(read_header(&rle_for_24bpp).is_none());
        rle_for_24bpp[30] = 0;
        assert!(read_header(&rle_for_24bpp).is_some());

        let mut compression3 = valid.clone();
        compression3[30] = 3;
        assert!(read_header(&compression3).is_none());

        let too_many_colours = windows_bmp(1, 1, 1, 0, &grey_palette(3), &[0; 4]);
        assert!(read_header(&too_many_colours).is_none());

        let mut short_header = valid.clone();
        short_header[14] = 11;
        assert!(read_header(&short_header).is_none());

        // The palette must end before the bitmap starts.
        let mut offset_inside_palette = valid.clone();
        offset_inside_palette[10] = 54;
        assert!(read_header(&offset_inside_palette).is_none());

        assert!(read_header(&valid[..20]).is_none());
        assert!(read_header(&[]).is_none());
    }

    /// Header sizes of 13 to 15 make the C++ code's unsigned header size wrap
    /// around, so it skips about 4 GiB and the palette ends past the bitmap.
    #[test]
    fn header_size_wraps_like_the_original() {
        let mut file = windows_bmp(1, 1, 8, 0, &grey_palette(1), &[0; 4]);
        file[14] = 13;
        assert!(read_header(&file).is_none());
    }

    #[test]
    fn decodes_8bpp_bottom_up_with_row_padding() {
        // Rows are stored bottom first and padded to four bytes.
        let pixels = [1, 2, 3, 0, 4, 5, 6, 0];
        let file = windows_bmp(3, 2, 8, 0, &grey_palette(8), &pixels);
        assert_eq!(decode(&file).unwrap(), [4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn decodes_24bpp_as_rgb() {
        let pixels = [1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0];
        let file = windows_bmp(2, 2, 24, 0, &[], &pixels);
        assert_eq!(
            decode(&file).unwrap(),
            [9, 8, 7, 12, 11, 10, 3, 2, 1, 6, 5, 4]
        );
    }

    #[test]
    fn decodes_4bpp_nibbles() {
        let pixels = [0x12, 0x34, 0, 0, 0x56, 0x78, 0, 0];
        let file = windows_bmp(4, 2, 4, 0, &grey_palette(16), &pixels);
        assert_eq!(decode(&file).unwrap(), [5, 6, 7, 8, 1, 2, 3, 4]);
    }

    #[test]
    fn decodes_1bpp_bits() {
        let pixels = [
            0b1010_0000,
            0b0000_0001,
            0,
            0,
            0b0101_0000,
            0b1000_0000,
            0,
            0,
        ];
        let file = windows_bmp(16, 2, 1, 0, &grey_palette(2), &pixels);
        let bitmap = decode(&file).unwrap();
        assert_eq!(&bitmap[..4], [0, 1, 0, 1]);
        assert_eq!(bitmap[8], 1);
        assert_eq!(&bitmap[16..20], [1, 0, 1, 0]);
        assert_eq!(bitmap[31], 1);
    }

    /// Rows are padded to four bytes after the last partly used byte. The
    /// original (and upstream OpenTTD) computed the padding from `width / 8`,
    /// rounded down, and so read every row after the first one byte late.
    #[test]
    fn pads_1bpp_rows_after_partial_bytes() {
        // Width 9 takes 2 bytes per row, so each row is padded with 2 bytes.
        let pixels = [0x80, 0x80, 0, 0, 0xFF, 0xFF, 0, 0];
        let file = windows_bmp(9, 2, 1, 0, &grey_palette(2), &pixels);
        let bitmap = decode(&file).unwrap();
        assert_eq!(&bitmap[9..], [1, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(&bitmap[..9], [1; 9]);
    }

    /// As above, for 4 bpp, where the original computed the padding from `width / 2`.
    #[test]
    fn pads_4bpp_rows_after_partial_bytes() {
        // Width 3 takes 2 bytes per row, so each row is padded with 2 bytes.
        let pixels = [0x12, 0x30, 0, 0, 0x45, 0x60, 0, 0];
        let file = windows_bmp(3, 2, 4, 0, &grey_palette(16), &pixels);
        assert_eq!(decode(&file).unwrap(), [4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn decodes_rle8() {
        let data = [
            3, 7, // run of three 7s
            0, 0, // end of line
            0, 3, 1, 2, 3, 0, // three literal pixels, padded to an even length
            0, 1, // end of bitmap
        ];
        let file = windows_bmp(4, 2, 8, 1, &grey_palette(8), &data);
        assert_eq!(decode(&file).unwrap(), [1, 2, 3, 0, 7, 7, 7, 0]);
    }

    #[test]
    fn rle8_delta_and_overlong_runs() {
        let data = [
            0, 2, 1, 1, // delta: one right, one up
            9, 5, // run longer than the rest of the row: excess ignored
            0, 1,
        ];
        let file = windows_bmp(3, 2, 8, 1, &grey_palette(8), &data);
        assert_eq!(decode(&file).unwrap(), [0, 5, 5, 0, 0, 0]);
    }

    #[test]
    fn decodes_rle4() {
        let data = [
            5, 0x12, // run of five pixels alternating 1 and 2
            0, 0, // end of line
            0, 3, 0x34, 0x50, // three literal pixels; two bytes, so no padding
            0, 1,
        ];
        let file = windows_bmp(5, 2, 4, 2, &grey_palette(16), &data);
        assert_eq!(decode(&file).unwrap(), [3, 4, 5, 0, 0, 1, 2, 1, 2, 1]);
    }

    #[test]
    fn invalid_rle_is_rejected() {
        // End of line on the last row.
        let file = windows_bmp(2, 1, 8, 1, &grey_palette(2), &[0, 0]);
        assert!(decode(&file).is_none());
        // Delta past the end of the row.
        let file = windows_bmp(2, 2, 8, 1, &grey_palette(2), &[0, 2, 2, 0]);
        assert!(decode(&file).is_none());
        // Delta above the top row.
        let file = windows_bmp(2, 2, 8, 1, &grey_palette(2), &[0, 2, 0, 2]);
        assert!(decode(&file).is_none());
        // Literal pixels past the end of the row.
        let file = windows_bmp(2, 1, 8, 1, &grey_palette(2), &[0, 3, 1, 2, 3, 0]);
        assert!(decode(&file).is_none());
        // Data ends before the bitmap is complete.
        let file = windows_bmp(2, 2, 8, 1, &grey_palette(2), &[2, 1]);
        assert!(decode(&file).is_none());
    }

    #[test]
    fn truncated_uncompressed_data_is_rejected() {
        let file = windows_bmp(4, 2, 24, 0, &[], &[0; 12]);
        assert!(decode(&file).is_none());
    }

    /// Reading past the end of the file yields zeros, as with `RandomAccessFile`:
    /// an 8 bpp row is only checked for the end of the file before it starts.
    #[test]
    fn reads_past_the_end_as_zeros() {
        let file = windows_bmp(4, 1, 8, 0, &grey_palette(8), &[5]);
        assert_eq!(decode(&file).unwrap(), [5, 0, 0, 0]);
    }

    #[test]
    fn rle_with_zero_height_is_rejected() {
        let file = windows_bmp(2, 0, 8, 1, &grey_palette(2), &[0, 1]);
        assert!(decode(&file).is_none());
    }
}
