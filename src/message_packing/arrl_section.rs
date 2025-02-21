use std::fmt::Display;

use snafu::Snafu;


const VALUE_TABLE: [&str; 86] = ["AB","AK","AL","AR","AZ","BC","CO","CT","DE","EB",      
                                 "EMA","ENY","EPA","EWA","GA","GH","IA","ID","IL","IN",
                                 "KS","KY","LA","LAX","NS","MB","MDC","ME","MI","MN",
                                 "MO","MS","MT","NC","ND","NE","NFL","NH","NL","NLI",       
                                 "NM","NNJ","NNY","TER","NTX","NV","OH","OK","ONE","ONN",
                                 "ONS","OR","ORG","PAC","PR","QC","RI","SB","SC","SCV",       
                                 "SD","SDG","SF","SFL","SJV","SK","SNJ","STX","SV","TN",       
                                 "UT","VA","VI","VT","WCF","WI","WMA","WNY","WPA","WTX",       
                                 "WV","WWA","WY","DX","PE","NB"];

pub struct ArrlSection {
    pub display_string: String,
    pub packed_bits: u16,
}

impl Display for ArrlSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string)
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Unable to parse \"{value}\" as a ArrlSection"))]
pub struct ParseArrlSectionError {
    value: String,
}

impl ArrlSection {
    pub fn try_from_string(string_value:&str) -> Result<Self, ParseArrlSectionError> {
        
        if let Some(position) = VALUE_TABLE.iter().position(|&v| v == string_value) {
            return Ok(ArrlSection {
                display_string: string_value.to_string(),
                packed_bits: position as u16 + 1
            });
        }

        todo!()
    }
}

// WI 1001100 76
#[cfg(test)]
mod tests {
    use super::*;

    mod with_wi {
        use super::*;

        #[test]
        fn display_string_is_correct() {
            let sec = ArrlSection::try_from_string("WI").unwrap();
            assert_eq!(sec.display_string, "WI");
        }

        #[test]
        fn packed_bits_are_correct() {
            let sec = ArrlSection::try_from_string("WI").unwrap();
            assert_eq!(sec.packed_bits, 0b1001100);
        }
    }
}