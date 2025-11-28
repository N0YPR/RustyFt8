//! A Priori (AP) Decoding Support
//!
//! Implements WSJT-X's AP decoding mechanism for weak signal decoding.
//! AP works by forcing certain bits to expected values during LDPC decoding.

use crate::message::{encode, CallsignHashCache};
use bitvec::prelude::*;

/// Standard message patterns for AP decoding (converted to ±1 for LLR use)
pub struct ApPatterns {
    /// "CQ" message prefix (29 bits)
    pub cq: [i8; 29],
    /// "CQ TEST" message prefix (29 bits)
    pub cq_test: [i8; 29],
    /// "CQ FD" (Field Day) message prefix (29 bits)
    pub cq_fd: [i8; 29],
    /// "RRR" message suffix (19 bits)
    pub rrr: [i8; 19],
    /// "73" message suffix (19 bits)
    pub msg_73: [i8; 19],
    /// "RR73" message suffix (19 bits)
    pub rr73: [i8; 19],
}

impl Default for ApPatterns {
    fn default() -> Self {
        // Standard patterns from WSJT-X ft8b.f90 lines 39-46
        // Original bits, then converted to ±1 (2*bit - 1)

        // CQ = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0]
        let cq = [-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1,-1,-1];

        // CQ TEST = [0,0,0,0,0,0,0,0,0,1,1,0,0,0,0,1,0,1,0,1,1,1,1,1,1,0,0,1,0]
        let cq_test = [-1,-1,-1,-1,-1,-1,-1,-1,-1,1,1,-1,-1,-1,-1,1,-1,1,-1,1,1,1,1,1,1,-1,-1,1,-1];

        // CQ FD = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,1,0,0,1,0,0,0,1,0]
        let cq_fd = [-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1,-1,-1,1,-1,-1,1,-1,-1,-1,1,-1];

        // RRR = [0,1,1,1,1,1,1,0,1,0,0,1,0,0,1,0,0,0,1]
        let rrr = [-1,1,1,1,1,1,1,-1,1,-1,-1,1,-1,-1,1,-1,-1,-1,1];

        // 73 = [0,1,1,1,1,1,1,0,1,0,0,1,0,1,0,0,0,0,1]
        let msg_73 = [-1,1,1,1,1,1,1,-1,1,-1,-1,1,-1,1,-1,-1,-1,-1,1];

        // RR73 = [0,1,1,1,1,1,1,0,0,1,1,1,0,1,0,1,0,0,1]
        let rr73 = [-1,1,1,1,1,1,1,-1,-1,1,1,1,-1,1,-1,1,-1,-1,1];

        Self {
            cq,
            cq_test,
            cq_fd,
            rrr,
            msg_73,
            rr73,
        }
    }
}

/// AP Type - different strategies for different message patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApType {
    /// Type 1: CQ ??? ??? (only CQ prefix known)
    CqAny = 1,
    /// Type 2: MyCall ??? ??? (user's callsign known)
    MyCallAny = 2,
    /// Type 3: MyCall DxCall ??? (both callsigns known)
    MyCallDxCallAny = 3,
    /// Type 4: MyCall DxCall RRR (full message with RRR)
    MyCallDxCallRrr = 4,
    /// Type 5: MyCall DxCall 73 (full message with 73)
    MyCallDxCall73 = 5,
    /// Type 6: MyCall DxCall RR73 (full message with RR73)
    MyCallDxCallRr73 = 6,
}

/// AP decoding configuration and state
pub struct ApDecoder {
    /// User's callsign (if configured)
    pub mycall: Option<String>,
    /// DX station's callsign (if configured)
    pub hiscall: Option<String>,
    /// Encoded bits for mycall+hiscall (58 bits: 28+1+28+1)
    pub apsym: Option<[i8; 58]>,
    /// Standard message patterns
    pub patterns: ApPatterns,
}

impl ApDecoder {
    /// Create a new AP decoder with user callsigns
    pub fn new(mycall: Option<String>, hiscall: Option<String>) -> Self {
        let mut decoder = Self {
            mycall: mycall.clone(),
            hiscall: hiscall.clone(),
            apsym: None,
            patterns: ApPatterns::default(),
        };

        // Generate AP symbols if callsigns are valid
        if let Some(ref my) = mycall {
            if my.len() >= 3 {
                decoder.generate_apsym();
            }
        }

        decoder
    }

    /// Generate AP symbols from mycall and hiscall
    ///
    /// Encodes "MyCall HisCall RRR" message and extracts first 58 bits
    /// (28 bits mycall + 1 bit i3.n3 + 28 bits hiscall + 1 bit i3.n3)
    fn generate_apsym(&mut self) {
        let mycall = match &self.mycall {
            Some(c) if c.len() >= 3 => c,
            _ => return,
        };

        // Use dummy hiscall if not provided (like WSJT-X does)
        let hiscall = self.hiscall.as_deref().unwrap_or("KA1ABC");

        // Construct message "MyCall HisCall RRR"
        let msg = format!("{} {} RRR", mycall, hiscall);

        // Encode using pack77
        let mut bits77 = bitvec![u8, Msb0; 0; 77];
        let mut cache = CallsignHashCache::new();

        if encode(&msg, &mut bits77, &mut cache).is_ok() {
            // Extract first 58 bits (callsigns only)
            let mut apsym = [0i8; 58];
            for i in 0..58 {
                // Convert 0/1 to -1/+1
                apsym[i] = if bits77[i] { 1 } else { -1 };
            }
            self.apsym = Some(apsym);
        }
    }

    /// Generate AP mask and LLR hints for a given AP type
    ///
    /// Returns (apmask, llr_hints):
    /// - apmask: 174-bit mask (true = known bit, false = unknown)
    /// - llr_hints: 174 f32 values with strong hints for known bits
    pub fn generate_ap_hints(&self, ap_type: ApType, llr_magnitude: f32) -> Option<(Vec<bool>, Vec<f32>)> {
        let mut apmask = vec![false; 174];
        let mut llr_hints = vec![0.0f32; 174];

        match ap_type {
            ApType::CqAny => {
                // Type 1: CQ ??? ???
                // Force bits 0-28 to CQ pattern
                for i in 0..29 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * self.patterns.cq[i] as f32;
                }
                // Force message type bits (i3.n3 = standard message)
                apmask[74] = true;
                apmask[75] = true;
                apmask[76] = true;
                llr_hints[74] = -llr_magnitude;  // bit 75 = 0
                llr_hints[75] = -llr_magnitude;  // bit 76 = 0
                llr_hints[76] = llr_magnitude;   // bit 77 = 1
            },

            ApType::MyCallAny => {
                // Type 2: MyCall ??? ???
                let apsym = self.apsym.as_ref()?;
                // Force bits 0-28 to mycall
                for i in 0..29 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * apsym[i] as f32;
                }
                // Force message type bits
                apmask[74] = true;
                apmask[75] = true;
                apmask[76] = true;
                llr_hints[74] = -llr_magnitude;
                llr_hints[75] = -llr_magnitude;
                llr_hints[76] = llr_magnitude;
            },

            ApType::MyCallDxCallAny => {
                // Type 3: MyCall DxCall ???
                let apsym = self.apsym.as_ref()?;
                // Only use if hiscall is configured
                if self.hiscall.is_none() {
                    return None;
                }
                // Force bits 0-57 to mycall+hiscall
                for i in 0..58 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * apsym[i] as f32;
                }
                // Force message type bits
                apmask[74] = true;
                apmask[75] = true;
                apmask[76] = true;
                llr_hints[74] = -llr_magnitude;
                llr_hints[75] = -llr_magnitude;
                llr_hints[76] = llr_magnitude;
            },

            ApType::MyCallDxCallRrr => {
                // Type 4: MyCall DxCall RRR (full message known)
                let apsym = self.apsym.as_ref()?;
                if self.hiscall.is_none() {
                    return None;
                }
                // Force bits 0-57 to mycall+hiscall
                for i in 0..58 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * apsym[i] as f32;
                }
                // Force bits 58-76 to RRR
                for i in 0..19 {
                    apmask[58 + i] = true;
                    llr_hints[58 + i] = llr_magnitude * self.patterns.rrr[i] as f32;
                }
            },

            ApType::MyCallDxCall73 => {
                // Type 5: MyCall DxCall 73 (full message known)
                let apsym = self.apsym.as_ref()?;
                if self.hiscall.is_none() {
                    return None;
                }
                // Force bits 0-57 to mycall+hiscall
                for i in 0..58 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * apsym[i] as f32;
                }
                // Force bits 58-76 to 73
                for i in 0..19 {
                    apmask[58 + i] = true;
                    llr_hints[58 + i] = llr_magnitude * self.patterns.msg_73[i] as f32;
                }
            },

            ApType::MyCallDxCallRr73 => {
                // Type 6: MyCall DxCall RR73 (full message known)
                let apsym = self.apsym.as_ref()?;
                if self.hiscall.is_none() {
                    return None;
                }
                // Force bits 0-57 to mycall+hiscall
                for i in 0..58 {
                    apmask[i] = true;
                    llr_hints[i] = llr_magnitude * apsym[i] as f32;
                }
                // Force bits 58-76 to RR73
                for i in 0..19 {
                    apmask[58 + i] = true;
                    llr_hints[58 + i] = llr_magnitude * self.patterns.rr73[i] as f32;
                }
            },
        }

        Some((apmask, llr_hints))
    }

    /// Get list of AP types to try based on configuration
    pub fn get_ap_passes(&self) -> Vec<ApType> {
        let mut passes = Vec::new();

        // Always try CQ pass (Type 1)
        passes.push(ApType::CqAny);

        // If mycall is configured, try MyCall passes
        if self.mycall.is_some() && self.apsym.is_some() {
            passes.push(ApType::MyCallAny);

            // If both callsigns configured, try full message passes
            if self.hiscall.is_some() {
                passes.push(ApType::MyCallDxCallAny);
                passes.push(ApType::MyCallDxCallRrr);
                passes.push(ApType::MyCallDxCall73);
                passes.push(ApType::MyCallDxCallRr73);
            }
        }

        passes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ap_patterns() {
        let patterns = ApPatterns::default();

        // CQ pattern should have 28 zeros and 1 one at position 26
        assert_eq!(patterns.cq[26], 1);
        assert_eq!(patterns.cq[0], -1);
    }

    #[test]
    fn test_ap_decoder_no_callsigns() {
        let decoder = ApDecoder::new(None, None);

        // Should still allow CQ pass
        let passes = decoder.get_ap_passes();
        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0], ApType::CqAny);
    }

    #[test]
    fn test_ap_decoder_with_mycall() {
        let decoder = ApDecoder::new(Some("K1BZM".to_string()), None);

        // Should allow CQ and MyCall passes
        let passes = decoder.get_ap_passes();
        assert!(passes.contains(&ApType::CqAny));
        assert!(passes.contains(&ApType::MyCallAny));
    }

    #[test]
    fn test_ap_hints_generation() {
        let decoder = ApDecoder::new(Some("K1BZM".to_string()), Some("EA3GP".to_string()));

        // Generate hints for Type 3
        let hints = decoder.generate_ap_hints(ApType::MyCallDxCallAny, 10.0);
        assert!(hints.is_some());

        let (mask, llrs) = hints.unwrap();
        assert_eq!(mask.len(), 174);
        assert_eq!(llrs.len(), 174);

        // First 58 bits should be masked
        assert_eq!(mask[0..58].iter().filter(|&&x| x).count(), 58);
        // Message type bits should be masked
        assert!(mask[74]);
        assert!(mask[75]);
        assert!(mask[76]);
    }
}
