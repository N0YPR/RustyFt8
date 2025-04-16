use std::fmt::Display;

use snafu::Snafu;

const VALUE_TABLE:[&str; 171] = ["AL","AK","AZ","AR","CA","CO","CT","DE","FL","GA",
                               "HI","ID","IL","IN","IA","KS","KY","LA","ME","MD",
                               "MA","MI","MN","MS","MO","MT","NE","NV","NH","NJ",
                               "NM","NY","NC","ND","OH","OK","OR","PA","RI","SC",
                               "SD","TN","TX","UT","VT","VA","WA","WV","WI","WY",
                               "NB","NS","QC","ON","MB","SK","AB","BC","NWT","NF",
                               "LB","NU","YT","PEI","DC","DR","FR","GD","GR","OV",
                               "ZH","ZL","X01","X02","X03","X04","X05","X06","X07","X08",
                               "X09","X10","X11","X12","X13","X14","X15","X16","X17","X18",
                               "X19","X20","X21","X22","X23","X24","X25","X26","X27","X28",
                               "X29","X30","X31","X32","X33","X34","X35","X36","X37","X38",
                               "X39","X40","X41","X42","X43","X44","X45","X46","X47","X48",
                               "X49","X50","X51","X52","X53","X54","X55","X56","X57","X58",
                               "X59","X60","X61","X62","X63","X64","X65","X66","X67","X68",
                               "X69","X70","X71","X72","X73","X74","X75","X76","X77","X78",
                               "X79","X80","X81","X82","X83","X84","X85","X86","X87","X88",
                               "X89","X90","X91","X92","X93","X94","X95","X96","X97","X98",
                               "X99"];

pub struct SerialNumberOrStateOrProvince {
    pub display_string: String,
    pub packed_bits: u16,
}

impl Display for SerialNumberOrStateOrProvince {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string)
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Unable to parse \"{value}\" as a SerialNumberOrStateOrProvince"))]
pub struct ParseSerialNumberOrStateOrProvinceError {
    value: String,
}

impl SerialNumberOrStateOrProvince {
    pub fn try_from_string(string_value:&str) -> Result<Self, ParseSerialNumberOrStateOrProvinceError> {
        if let Ok(value) = string_value.parse::<u16>() {
            if value > 0 && value < 8000 {
                return Ok(SerialNumberOrStateOrProvince {
                    display_string: string_value.to_string(),
                    packed_bits: value
                });
            } else {
                return Err(ParseSerialNumberOrStateOrProvinceError { value: string_value.to_string() });
            }
        }

        if let Some(position) = VALUE_TABLE.iter().position(|&v| v == string_value) {
            return Ok(SerialNumberOrStateOrProvince {
                display_string: string_value.to_string(),
                packed_bits: 8001 + position as u16
            });
        }

        return Err(ParseSerialNumberOrStateOrProvinceError { value: string_value.to_string() });
    }

    pub fn try_from_packed_bits(value:u16) -> Result<Self, ParseSerialNumberOrStateOrProvinceError> {
        let value_string:String;
        if value >= 8001 && value <= 8001 + VALUE_TABLE.len() as u16 {
            value_string = VALUE_TABLE[(value - 8001) as usize].to_string();
        } else if value >= 1 && value <= 8000 {
            value_string = format!("{value:0>4}");
        } else {
            return Err(ParseSerialNumberOrStateOrProvinceError { value: value.to_string() });
        }

        Ok(SerialNumberOrStateOrProvince {
            display_string: value_string,
            packed_bits: value
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_success {
        ($name:ident, $message:expr, $expected_message:expr, $expected_bits:expr) => {
            paste::item! {
                mod [< $name:lower >] {
                    use super::*;

                    mod when_try_from_string {
                        use super::*;

                        #[test]
                        fn display_string_is_correct() {
                            let ser = SerialNumberOrStateOrProvince::try_from_string($message).unwrap();
                            assert_eq!(format!("{ser}"), $expected_message);
                        }

                        #[test]
                        fn packed_bits_are_correct() {
                            let ser = SerialNumberOrStateOrProvince::try_from_string($message).unwrap();
                            assert_eq!(ser.packed_bits, $expected_bits);
                            
                        }
                    }

                    mod when_try_from_packed_bits {
                        use super::*;

                        #[test]
                        fn display_string_is_correct() {
                            let ser = SerialNumberOrStateOrProvince::try_from_packed_bits($expected_bits).unwrap();
                            assert_eq!(format!("{ser}"), $expected_message);
                        }

                        #[test]
                        fn packed_bits_are_correct() {
                            let ser = SerialNumberOrStateOrProvince::try_from_packed_bits($expected_bits).unwrap();
                            assert_eq!(ser.packed_bits, $expected_bits);
                            
                        }
                    }
                }
            }
        }
    }

    test_success!(with_0001, "0001", "0001", 0b0000000000001);
    test_success!(with_7999, "7999", "7999", 0b1111100111111);
    test_success!(with_al, "AL", "AL", 0b1111101000001);
    test_success!(with_wi, "WI", "WI", 0b1111101110001);
    test_success!(with_x99, "X99", "X99", 0b1111111101011);
}