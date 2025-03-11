pub const FT8_CHAR_TABLE_FULL:&str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
pub const FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH:&str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/";
pub const FT8_CHAR_TABLE_ALPHANUM_SPACE:&str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const FT8_CHAR_TABLE_ALPHA_SPACE:&str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const FT8_CHAR_TABLE_GRIDSQUARE_ALPHA:&str = "ABCDEFGHIJKLMNOPQR";
pub const FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_LOWER:&str = "abcdefghijklmnopqrstuvwx";
pub const FT8_CHAR_TABLE_ALPHANUM:&str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const FT8_CHAR_TABLE_NUMERIC:&str = "0123456789";

// https://github.com/vk3jpk/ft8-notes/blob/master/ft8.py#L29
// https://gist.github.com/NT7S/6e38d8a35d153f015d476bc49b40effb
// FT-8 CRC-14 polynomial without the leading (MSB) 1
// x^14 + x^13 + x^10 + x^9 + x^8 + x^6 + x^4 + x^2 + x^1 + 1
// 110011101010111
// drop msb
// 10011101010111
// 0x2757
pub const CRC_POLYNOMIAL:u16 = 0x2757;

pub const FT8_GRAY_CODE: [u8; 8] = [0, 1, 3, 2, 5, 6, 4, 7];

pub const TONE_COUNT: usize = 8;
pub const SYMBOL_RATE: f32 = 6.25;
pub const SAMPLE_RATE: f32 = 12_000.0;
pub const TONE_SPACING: f32 = 6.25;
pub const CHANNEL_SYMBOLS_COUNT: usize = 79;
pub const FT8_COSTAS: [u8; 7] = [3,1,4,0,6,5,2];