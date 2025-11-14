/// FT8 Protocol Character Set Constants
///
/// This module contains all character sets used in the FT8 protocol for various encoding schemes.

// Text encoding character sets (base-N encoding)

/// Base-42 character set for Type 0 free text messages (13 characters, 71 bits)
/// Used for encoding messages like "TNX BOB 73 GL"
/// Character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?'
pub const CHARSET_BASE42: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";

/// Base-38 character set for Type 4 NonStandardCall messages (11 characters, 58 bits)
/// Used for encoding compound callsigns like "PJ4/K1ABC" or "KH1/KH7Z"
/// Character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/'
pub const CHARSET_BASE38: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/";

// Standard callsign encoding character sets (28-bit pack28/unpack28)

/// Character set for first position in standard callsigns (space + 0-9 + A-Z)
/// Total: 37 characters
pub const CHARSET_A1: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Character set for second position in standard callsigns (0-9 + A-Z)
/// Total: 36 characters
pub const CHARSET_A2: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Character set for digit position in standard callsigns (0-9)
/// Total: 10 characters
pub const CHARSET_A3: &str = "0123456789";

/// Character set for letter-only positions in standard callsigns (space + A-Z)
/// Total: 27 characters
pub const CHARSET_A4: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ";

// Protocol limits

/// Number of special tokens (CQ variants, DE, QRZ) in the WSJT-X protocol
pub const NTOKENS: u32 = 2063592;

/// Maximum 22-bit hash value for non-standard callsigns
pub const MAX22: u32 = 4194304;

/// Maximum grid square value (18*18*10*10 = 32400)
/// Values above this are signal reports and special codes
pub const MAXGRID4: u16 = 32400;
