use alloc::string::String;

/// Internal representation of parsed message components
#[derive(Debug, Clone, PartialEq)]
pub enum MessageVariant {
    /// Standard Type 1 (i3=1) or Type 2 (i3=2) message: two callsigns with optional grid/report
    /// 
    /// Structure:
    /// - n28a (28 bits): First callsign
    /// - ipa (1 bit): /P or /R suffix for first callsign
    /// - n28b (28 bits): Second callsign
    /// - ipb (1 bit): /P or /R suffix for second callsign
    /// - ir (1 bit): R/acknowledge flag
    /// - igrid4 (15 bits): Grid square or report
    /// - i3 (3 bits): Message type = 1 (standard) or 2 (compound callsign)
    ///
    /// Examples:
    /// - "CQ N0YPR DM42" - CQ with grid (Type 1)
    /// - "CQ N0YPR/R DM42" - CQ with rover suffix (Type 1)
    /// - "CQ G4ABC/P IO91" - CQ with portable suffix (Type 2, compound callsign)
    /// - "N0YPR W1ABC +05" - Signal report
    /// - "N0YPR W1ABC R-10" - Signal report with R flag
    /// - "W1ABC N0YPR RR73" - Final acknowledgment
    /// - "W1ABC N0YPR 73" - Sign-off
    Standard {
        /// n28a: First callsign (or special callsign like "CQ", "DE") - bits 0-27
        call1: String,
        /// ipa: /P or /R suffix for first callsign - bit 28
        call1_suffix: bool,
        /// n28b: Second callsign - bits 29-56
        call2: String,
        /// ipb: /P or /R suffix for second callsign - bit 57
        call2_suffix: bool,
        /// ir: R/acknowledge flag - bit 58
        r_flag: bool,
        /// igrid4: Grid square or report (e.g., "DM42", "+05", "R-10", "RRR", "RR73", "73", or blank) - bits 59-73
        grid_or_report: String,
    },
    
    /// EU VHF Contest Type 2 message (i3=2) for portable operations with /P suffix
    /// 
    /// This is the FT8 Type 2 message format used for EU VHF contests where
    /// stations operate portable with /P suffix. Structure is identical to Type 1
    /// but uses i3=2 and suffix flags indicate /P instead of /R.
    ///
    /// Note: This is distinct from Type 0.2 (legacy, disabled) and Type 5 (future).
    ///
    /// Structure:
    /// - n28a (28 bits): First callsign encoding (CQ, CQ modifier, or standard callsign)
    /// - ipa (1 bit): /P suffix flag for first callsign
    /// - n28b (28 bits): Second callsign encoding
    /// - ipb (1 bit): /P suffix flag for second callsign
    /// - ir (1 bit): R/acknowledge flag
    /// - igrid4 (15 bits): Grid square or signal report
    /// - i3 (3 bits): Message type = 2
    ///
    /// Examples:
    /// - "CQ G4ABC/P IO91" - CQ call from portable station
    /// - "G4ABC/P PA9XYZ JO22" - QSO with portable station
    /// - "PA9XYZ G4ABC/P RR73" - Reply to portable station
    EuVhfContestType2 {
        /// n28a: First callsign - bits 0-27
        call1: String,
        /// ipa: /P suffix for first callsign - bit 28
        call1_suffix: bool,
        /// n28b: Second callsign - bits 29-56
        call2: String,
        /// ipb: /P suffix for second callsign - bit 57
        call2_suffix: bool,
        /// ir: R/acknowledge flag - bit 58
        r_flag: bool,
        /// igrid4: Grid square or report - bits 59-73
        grid_or_report: String,
    },
    
    /// ARRL RTTY Roundup message (i3=3) for contest exchanges
    /// 
    /// This is the FT8 Type 3 message format used for ARRL RTTY Roundup contests.
    /// 
    /// Structure:
    /// - tu (1 bit): "TU;" prefix flag
    /// - n28a (28 bits): First callsign encoding
    /// - n28b (28 bits): Second callsign encoding
    /// - r (1 bit): "R" acknowledgment flag
    /// - rst (3 bits): Signal report (always 5X9, encoded as index)
    /// - nexch (13 bits): Exchange - serial (1-7999) or state/province (8000+index)
    /// - i3 (3 bits): Message type = 3
    ///
    /// Examples:
    /// - "K1ABC W9XYZ 579 WI" - Exchange with state (WI = Wisconsin)
    /// - "W9XYZ K1ABC R 589 MA" - Reply with R flag and state
    /// - "TU; KA0DEF K1ABC R 569 MA" - With TU prefix
    /// - "K1ABC W9XYZ 559 0013" - DX exchange with serial number
    RttyRoundup {
        /// tu: "TU;" prefix flag - bit 0
        tu: bool,
        /// n28a: First callsign - bits 1-28
        call1: String,
        /// n28b: Second callsign - bits 29-56
        call2: String,
        /// r: "R" acknowledgment flag - bit 57
        r_flag: bool,
        /// rst: Signal report middle digit (0-7 representing 2-9 in 5X9 format) - bits 58-60
        rst: u8,
        /// nexch: Exchange (state/province or serial) - bits 61-73
        exchange: String,
    },
    
    /// Free text Type 0.0 (i3=0, n3=0) message: arbitrary text up to 13 characters
    /// 
    /// Structure:
    /// - text (71 bits): Up to 13 characters encoded using base-42
    /// - n3 (3 bits): Subtype = 0
    /// - i3 (3 bits): Message type = 0
    ///
    /// Character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?'
    ///
    /// Examples:
    /// - "TNX BOB 73 GL" - Thank you message
    /// - "HELLO WORLD" - Simple greeting
    FreeText {
        /// text: Arbitrary text message (up to 13 characters) - bits 0-70
        text: String,
    },
    
    /// DXpedition Type 0.1 (i3=0, n3=1) message: special format for DXpedition operations
    /// 
    /// Structure:
    /// - n28a (28 bits): First callsign (must be valid standard callsign)
    /// - n28b (28 bits): Second callsign (must be valid standard callsign)
    /// - n10 (10 bits): 10-bit hash of non-standard callsign in angle brackets
    /// - n5 (5 bits): Signal report encoded as (report+30)/2, range -30 to +32 dB
    /// - n3 (3 bits): Subtype = 1
    /// - i3 (3 bits): Message type = 0
    ///
    /// Examples:
    /// - "K1ABC RR73; W9XYZ <KH1/KH7Z> -08"
    /// - "N0YPR RR73; KK7JXP <PJ4/N0YPR> +15"
    ///
    /// The format is always: CALL1 RR73; CALL2 <HASHCALL> REPORT
    /// where HASHCALL is a non-standard callsign referenced via 10-bit hash
    DXpedition {
        /// n28a: First callsign - bits 0-27
        call1: String,
        /// n28b: Second callsign - bits 28-55
        call2: String,
        /// n10: 10-bit hash of callsign in angle brackets - bits 56-65
        hash_call: String,
        /// n5: Signal report (-30 to +32 dB) - bits 66-70
        report: i8,
    },
    
    /// ARRL Field Day Type 0.3/0.4 (i3=0, n3=3 or n3=4) message
    /// 
    /// Structure:
    /// - n28a (28 bits): First callsign (must be valid standard callsign)
    /// - n28b (28 bits): Second callsign (must be valid standard callsign)
    /// - ir (1 bit): R/acknowledge flag
    /// - intx (4 bits): Number of transmitters - 1 (or - 17 for n3=4)
    /// - nclass (3 bits): Class letter (0=A, 1=B, ..., 5=F)
    /// - isec (7 bits): ARRL section code (1-86)
    /// - n3 (3 bits): Subtype = 3 (for 1-16 transmitters) or 4 (for 17-32 transmitters)
    /// - i3 (3 bits): Message type = 0
    ///
    /// Examples:
    /// - "K1ABC W9XYZ 6A WI" - 6 transmitters, class A, Wisconsin section
    /// - "W9XYZ K1ABC R 17B EMA" - 17 transmitters, class B, Eastern Massachusetts, with R flag
    ///
    /// The format is: CALL1 CALL2 [R] <NUM><CLASS> <SECTION>
    /// where NUM is 1-32, CLASS is A-F, and SECTION is a 2-3 letter code
    FieldDay {
        /// n28a: First callsign - bits 0-27
        call1: String,
        /// n28b: Second callsign - bits 28-55
        call2: String,
        /// ir: R/acknowledge flag - bit 56
        r_flag: bool,
        /// intx: Encoded transmitter count (actual count = intx+1 or intx+17) - bits 57-60
        intx: u8,
        /// nclass: Class letter (0=A, 1=B, 2=C, 3=D, 4=E, 5=F) - bits 61-63
        nclass: u8,
        /// isec: ARRL section code (1-86) - bits 64-70
        isec: u8,
        /// n3: Subtype (3 for 1-16 TX, 4 for 17-32 TX) - bits 71-73
        n3: u8,
    },
    
    /// Telemetry Type 0.5 (i3=0, n3=5) message: 18 hexadecimal digits
    /// 
    /// Structure:
    /// - ntel1 (23 bits): First 6 hex digits (0x000000 to 0x7FFFFF)
    /// - ntel2 (24 bits): Next 6 hex digits (0x000000 to 0xFFFFFF)
    /// - ntel3 (24 bits): Last 6 hex digits (0x000000 to 0xFFFFFF)
    /// - n3 (3 bits): Subtype = 5
    /// - i3 (3 bits): Message type = 0
    ///
    /// Examples:
    /// - "123456789ABCDEF012" - 18 hex digits
    /// - "0123456789abcdef01" - Can use lowercase
    ///
    /// The format is 18 hexadecimal digits (0-9, A-F, case insensitive).
    /// Leading zeros can be omitted and the message will be right-aligned.
    Telemetry {
        /// hex_string: 18 hexadecimal digits - bits 0-70
        hex_string: String,
    },
    
    /// NonStandardCall Type 1.4 (i3=1, n3=4) message: for compound callsigns
    /// 
    /// Structure:
    /// - i3 (3 bits): Message type = 1
    /// - n3 (3 bits): Subtype = 4
    /// - n12 (12 bits): 12-bit hash of the compound callsign
    /// - c58 (58 bits): Encoded text (up to 10 characters) containing full message
    /// - i3 (3 bits): Message type = 1 (repeated)
    ///
    /// Examples:
    /// - "CQ KH1/KH7Z" - CQ call with compound callsign
    /// - "CQ PJ4/K1ABC" - CQ call with DX prefix
    ///
    /// This format transmits the full callsign text so receivers can cache it.
    /// The 12-bit hash allows future messages to reference this callsign via cache lookup.
    NonStandardCall {
        /// text: Full message text (up to 10 characters) - encoded in c58 field
        text: String,
    },
    
    // Future variants:
    // WSPR { ... },                // i3=0, n3=6
}
