#[cfg(test)]
mod tests {
    use crate::{constants::CHANNEL_SYMBOLS_COUNT, message::{message_parse_error::MessageParseError, Message}, util::bitvec_utils::PackBitvecFieldType};

    use super::*;

    #[test]
    fn from_channel_symbols_invalid_length() {
        let channel_symbols = vec![0u8];
        let result = Message::try_from(channel_symbols.as_slice());
        assert!(matches!(
            result,
            Err(MessageParseError::InvalidSymbolsLength)
        ));
    }

    #[test]
    fn from_channel_symbols_invalid_costas() {
        let channel_symbols = vec![0u8; CHANNEL_SYMBOLS_COUNT];
        let result = Message::try_from(channel_symbols.as_slice());
        assert!(matches!(result, Err(MessageParseError::InvalidSymbols)));
    }

    #[test]
    fn from_channel_symbols_display_string_matches() {
        let channel_symbols_str =
            "3140652000671215006116571652175530543140652375421655752603157715414212433140652";
        let channel_symbols: Vec<u8> = channel_symbols_str
            .chars()
            .map(|c| {
                c.to_digit(8) // Parse as octal digit (0-7)
                    .expect("Input contains invalid characters") as u8
            })
            .collect();

        let message = Message::try_from(channel_symbols.as_slice())
            .expect("Failed to convert symbols to Message");

        assert_eq!(message.display_string, "CQ SOTA N0YPR/R DM42");
    }

    macro_rules! assert_parse_successfully {
        ($name:ident, $message:expr, $expected_message:expr, $channel_symbols_str:expr) => {
            paste::item! {
                mod [< with_ $name:lower >] {
                    use super::*;

                    #[test]
                    fn packed_string_is_correct() {
                        let m = Message::try_from($message).unwrap();
                        assert_eq!(format!("{m}"), $expected_message);
                    }

                    #[test]
                    fn packed_bits_are_correct() {
                        let message = Message::try_from($message).unwrap().message;
                        assert_eq!(message, extract_message_from_symbols_str($channel_symbols_str));
                    }

                    #[test]
                    fn crc_is_correct() {
                        let message = Message::try_from($message).unwrap();
                        let crc = message.checksum;

                        assert_eq!(crc, extract_crc_bits_from_symbols_str($channel_symbols_str));
                    }

                    #[test]
                    fn parity_is_correct() {
                        let message = Message::try_from($message).unwrap();
                        assert_eq!(message.parity, extract_parity_from_symbols_str($channel_symbols_str));
                    }

                    #[test]
                    fn channel_symbols_are_correct() {
                        let message = Message::try_from($message).unwrap();
                        let channel_symbols:String = message.channel_symbols.iter().map(|b| (b + b'0') as char).collect();
                        assert_eq!(channel_symbols, $channel_symbols_str);
                    }
                }
            }
        }
    }

    mod wsjtx_tests {

        use bitvec::prelude::*;

        use crate::{message::gray::GrayCode, util::bitvec_utils::bitvec_to_u128};

        use super::*;

        fn extract_gray_decoded_message_symbols_from_symbols_str(symbols_str: &str) -> Vec<u8> {
            if symbols_str.len() != 79 {
                panic!("Input string must be 79 characters long.");
            }

            let symbols_str_without_costas =
                format!("{}{}", &symbols_str[7..36], &symbols_str[43..72]);

            let symbols: Vec<u8> = symbols_str_without_costas
                .chars()
                .map(|c| {
                    c.to_digit(8) // Parse as octal digit (0-7)
                        .expect("Input contains invalid characters") as u8
                })
                .collect();

            let gray = GrayCode::new();
            let gray_decoded_symbols = gray.decode(&symbols);

            gray_decoded_symbols
        }

        fn extract_message_from_symbols_str(symbols_str: &str) -> u128 {
            let symbols = extract_gray_decoded_message_symbols_from_symbols_str(symbols_str);

            let mut bitvec: BitVec<u8, Msb0> = BitVec::new();
            for &symbol in symbols.iter().take(26) {
                for i in (0..3).rev() {
                    // Extract bits from most significant to least
                    let bit = (symbol >> i) & 1 != 0; // Convert to boolean
                    bitvec.push(bit);
                }
            }
            // remove the very last bit since it was extra... 3*26=78.. only needed 77
            bitvec.remove(bitvec.len() - 1);

            let message = bitvec_to_u128(&bitvec, 77);

            message
        }

        fn extract_crc_bits_from_symbols_str(symbols_str: &str) -> u16 {
            let symbols = extract_gray_decoded_message_symbols_from_symbols_str(symbols_str);

            let mut crc_bits: u16 = 0;
            for &symbol in symbols.iter().skip(25).take(6) {
                let lowest_three_bits = symbol & 0b111;
                crc_bits = crc_bits << 3;
                crc_bits |= lowest_three_bits as u16;
            }
            crc_bits = (crc_bits >> 2) & 0b11111111111111;

            crc_bits
        }

        fn extract_parity_from_symbols_str(symbols_str: &str) -> u128 {
            let symbols = extract_gray_decoded_message_symbols_from_symbols_str(symbols_str);

            // crc_bits
            // codeword is 174 bits long, need the last 83bits
            let mut bitvec: BitVec<u8, Msb0> = BitVec::new();
            for &symbol in symbols.iter() {
                symbol.pack_into_bitvec(&mut bitvec, 3);
            }
            let parity_bits = &bitvec[91..];

            let mut parity = 0u128;
            for bit in parity_bits {
                parity = (parity << 1) | (*bit as u128);
            }

            parity
        }

        // all of these tests are from wsjtx source code
        // src/wsjtx/lib/ft8/ft8_testmsg.f90
        // ran through ft8sim to determine the expected output for the tests below
        // example:
        // $ build/wsjtx-prefix/src/wsjtx-build/ft8sim "TNX BOB 73 GL" 1500 0 0 0 1 -10
        //   Decoded message: TNX BOB 73 GL                           i3.n3: 0.0
        //   f0: 1500.000   DT:  0.00   TxT:  12.6   SNR: -10.0  BW:50.0

        //   Message bits:
        //   01100011111011011100111011100010101001001010111000000111111101010000000000000

        //   Channel symbols:
        //   3140652207447147063336401773500017703140652646427306546072440503670130533140652
        //      1   0.00 1500.00  -10.0  000000_000001.wav   -9.99

        assert_parse_successfully!(
            wsjtx_1,
            "TNX BOB 73 GL",
            "TNX BOB 73 GL",
            "3140652207447147063336401773500017703140652646427306546072440503670130533140652"
        );
        assert_parse_successfully!(
            wsjtx_2,
            "K1ABC RR73; W9XYZ <KH1/KH7Z> -08",
            "K1ABC RR73; W9XYZ <KH1/KH7Z> -08",
            "3140652032247523515133264021134317153140652027407072730041362310127254663140652"
        );
        assert_parse_successfully!(
            wsjtx_3,
            "PA9XYZ 590003 IO91NP",
            "PA9XYZ 590003",
            "3140652362572673220023744672445005373140652010420711215646670140364610753140652"
        );
        assert_parse_successfully!(
            wsjtx_4,
            "G4ABC/P R 570007 JO22DB",
            "G4ABC/P R 570",
            "3140652167706375165046001437733003363140652220745304647271234314310031673140652"
        );
        assert_parse_successfully!(
            wsjtx_5,
            "K1ABC W9XYZ 6A WI",
            "K1ABC W9XYZ 6A WI",
            "3140652032247523515133264035320405303140652101020166700026554505077720623140652"
        );
        assert_parse_successfully!(
            wsjtx_6,
            "W9XYZ K1ABC R 17B EMA",
            "W9XYZ K1ABC R 17B EMA",
            "3140652020355725011672416200537013033140652330677001403444125317721563223140652"
        );
        assert_parse_successfully!(
            wsjtx_7,
            "123456789ABCDEF012",
            "123456789ABCDEF012",
            "3140652110453657532367167240056304313140652620633153646703256576437647343140652"
        );
        assert_parse_successfully!(
            wsjtx_8,
            "CQ K1ABC FN42",
            "CQ K1ABC FN42",
            "3140652000000001005476704606021533433140652736011047517007334745455133543140652"
        );
        assert_parse_successfully!(
            wsjtx_9,
            "K1ABC W9XYZ EN37",
            "K1ABC W9XYZ EN37",
            "3140652032247523504061147005134325373140652464557561564770300376175462233140652"
        );
        assert_parse_successfully!(
            wsjtx_10,
            "W9XYZ K1ABC -11",
            "W9XYZ K1ABC -11",
            "3140652020355725005476704617463024063140652536316515751700077044377507213140652"
        );
        assert_parse_successfully!(
            wsjtx_11,
            "K1ABC W9XYZ R-09",
            "K1ABC W9XYZ R-09",
            "3140652032247523504061147027463527033140652323406130213743267634453040613140652"
        );
        assert_parse_successfully!(
            wsjtx_12,
            "W9XYZ K1ABC RRR",
            "W9XYZ K1ABC RRR",
            "3140652020355725005476704617455530313140652564305535161117524523127753273140652"
        );
        assert_parse_successfully!(
            wsjtx_13,
            "K1ABC W9XYZ 73",
            "K1ABC W9XYZ 73",
            "3140652032247523504061147017456023753140652176074113361533126044715626273140652"
        );
        assert_parse_successfully!(
            wsjtx_14,
            "K1ABC W9XYZ RR73",
            "K1ABC W9XYZ RR73",
            "3140652032247523504061147017426332613140652071301161600346511151226424023140652"
        );
        assert_parse_successfully!(
            wsjtx_15,
            "CQ FD K1ABC FN42",
            "CQ FD K1ABC FN42",
            "3140652000001110505476704606021533743140652551744705346540117264367236423140652"
        );
        assert_parse_successfully!(
            wsjtx_16,
            "CQ TEST K1ABC/R FN42",
            "CQ TEST K1ABC/R FN42",
            "3140652000406275505476704656021522243140652712131455071561243646177737743140652"
        );
        assert_parse_successfully!(
            wsjtx_17,
            "K1ABC/R W9XYZ EN37",
            "K1ABC/R W9XYZ EN37",
            "3140652032247523404061147005134332153140652623707512241501513760247527103140652"
        );
        assert_parse_successfully!(
            wsjtx_18,
            "W9XYZ K1ABC/R R FN42",
            "W9XYZ K1ABC/R R FN42",
            "3140652020355725005476704646021534063140652447233323457764637506512367623140652"
        );
        assert_parse_successfully!(
            wsjtx_19,
            "K1ABC/R W9XYZ RR73",
            "K1ABC/R W9XYZ RR73",
            "3140652032247523404061147017426325433140652216151112327115302545134563313140652"
        );
        assert_parse_successfully!(
            wsjtx_20,
            "CQ TEST K1ABC FN42",
            "CQ TEST K1ABC FN42",
            "3140652000406275505476704606021520133140652212501560611771401652231035343140652"
        );
        assert_parse_successfully!(
            wsjtx_21,
            "W9XYZ <PJ4/K1ABC> -11",
            "W9XYZ <PJ4/K1ABC> -11",
            "3140652020355725001633651317463025333140652721702305367726741577047037163140652"
        );
        assert_parse_successfully!(
            wsjtx_22,
            "<PJ4/K1ABC> W9XYZ R-09",
            "<PJ4/K1ABC> W9XYZ R-09",
            "3140652004613406004061147027463523403140652700266426703075361110173346223140652"
        );
        assert_parse_successfully!(
            wsjtx_23,
            "CQ W9XYZ EN37",
            "CQ W9XYZ EN37",
            "3140652000000001004061147005134327023140652527476570561660640101346156613140652"
        );
        assert_parse_successfully!(
            wsjtx_24,
            "<YW18FIFA> W9XYZ -11",
            "<YW18FIFA> W9XYZ -11",
            "3140652006230634004061147017463025173140652301501240633504530456107701703140652"
        );
        assert_parse_successfully!(
            wsjtx_25,
            "W9XYZ <YW18FIFA> R-09",
            "W9XYZ <YW18FIFA> R-09",
            "3140652020355725001345136527463535673140652666243061260136572121271345123140652"
        );
        assert_parse_successfully!(
            wsjtx_26,
            "<YW18FIFA> KA1ABC",
            "<YW18FIFA> KA1ABC",
            "3140652006230634113704355117455325073140652112346203553211534271220352553140652"
        );
        assert_parse_successfully!(
            wsjtx_27,
            "KA1ABC <YW18FIFA> -11",
            "KA1ABC <YW18FIFA> -11",
            "3140652562521330501345136517463035563140652745336500352710660271112473543140652"
        );
        assert_parse_successfully!(
            wsjtx_28,
            "<YW18FIFA> KA1ABC R-17",
            "<YW18FIFA> KA1ABC R-17",
            "3140652006230634113704355127460530343140652746043101745421563500056465063140652"
        );
        assert_parse_successfully!(
            wsjtx_29,
            "<YW18FIFA> KA1ABC 73",
            "<YW18FIFA> KA1ABC 73",
            "3140652006230634113704355117456020263140652673662145153445157102313527513140652"
        );
        assert_parse_successfully!(
            wsjtx_30,
            "CQ G4ABC/P IO91",
            "CQ G4ABC/P IO91",
            "3140652000000001005515065457405456273140652311753555773213266103254602113140652"
        );
        assert_parse_successfully!(
            wsjtx_31,
            "G4ABC/P PA9XYZ JO22",
            "G4ABC/P PA9XYZ JO22",
            "3140652033040342222473413510546556673140652125365204412473533331244335523140652"
        );
        assert_parse_successfully!(
            wsjtx_32,
            "PA9XYZ G4ABC/P RR73",
            "PA9XYZ G4ABC/P RR73",
            "3140652667262063005515065467426366703140652155174750577504502006433672343140652"
        );
        assert_parse_successfully!(
            wsjtx_33,
            "K1ABC W9XYZ 579 WI",
            "K1ABC W9XYZ 579 WI",
            "3140652011672416304061147037725347523140652306512463403404071636453510363140652"
        );
        assert_parse_successfully!(
            wsjtx_34,
            "W9XYZ K1ABC R 589 MA",
            "W9XYZ K1ABC R 589 MA",
            "3140652015133264005476704672736370703140652556231412670171422210666331723140652"
        );
        assert_parse_successfully!(
            wsjtx_35,
            "K1ABC KA0DEF 559 MO",
            "K1ABC KA0DEF 559 MO",
            "3140652011672416213703052617734344213140652530733115357714754126135471623140652"
        );
        assert_parse_successfully!(
            wsjtx_36,
            "TU; KA0DEF K1ABC R 569 MA",
            "TU; KA0DEF K1ABC R 569 MA",
            "3140652436405107305476704642736345103140652330752307172673211532446754253140652"
        );
        assert_parse_successfully!(
            wsjtx_37,
            "KA1ABC G3AAA 529 0013",
            "KA1ABC G3AAA 529 0013",
            "3140652336415610305507315400002343163140652702747234356765754244623420063140652"
        );
        assert_parse_successfully!(
            wsjtx_38,
            "TU; G3AAA K1ABC R 559 MA",
            "TU; G3AAA K1ABC R 559 MA",
            "3140652511014521505476704667736374673140652147443157301235307742101161613140652"
        );
        assert_parse_successfully!(
            wsjtx_39,
            "CQ KH1/KH7Z",
            "CQ KH1/KH7Z",
            "3140652155400000317016042650330214403140652246332541464425542473300211553140652"
        );
        assert_parse_successfully!(
            wsjtx_40,
            "CQ PJ4/K1ABC",
            "CQ PJ4/K1ABC",
            "3140652366200016073153143630005210413140652661416746414647456323744275423140652"
        );
        assert_parse_successfully!(
            wsjtx_41,
            "PJ4/K1ABC <W9XYZ>",
            "PJ4/K1ABC <W9XYZ>",
            "3140652754100016073153143630004104403140652260770176145261322551452103013140652"
        );
        assert_parse_successfully!(
            wsjtx_42,
            "<W9XYZ> PJ4/K1ABC RRR",
            "<W9XYZ> PJ4/K1ABC RRR",
            "3140652754100016073153143630005614063140652361206660067077171261117407013140652"
        );
        assert_parse_successfully!(
            wsjtx_43,
            "PJ4/K1ABC <W9XYZ> 73",
            "PJ4/K1ABC <W9XYZ> 73",
            "3140652754100016073153143630007611403140652310172166217632341002174415723140652"
        );
        assert_parse_successfully!(
            wsjtx_44,
            "<W9XYZ> YW18FIFA",
            "<W9XYZ> YW18FIFA",
            "3140652754100000264707174620325114443140652126305246567642322733274461643140652"
        );
        assert_parse_successfully!(
            wsjtx_45,
            "YW18FIFA <W9XYZ> RRR",
            "YW18FIFA <W9XYZ> RRR",
            "3140652754100000264707174620324604023140652025471750645456171023533165643140652"
        );
        assert_parse_successfully!(
            wsjtx_46,
            "<W9XYZ> YW18FIFA 73",
            "<W9XYZ> YW18FIFA 73",
            "3140652754100000264707174620326601443140652076507256435211341240552157353140652"
        );
        assert_parse_successfully!(
            wsjtx_47,
            "CQ YW18FIFA",
            "CQ YW18FIFA",
            "3140652124100000264707174620325205033140652432356364551041722633453063573140652"
        );
        assert_parse_successfully!(
            wsjtx_48,
            "<KA1ABC> YW18FIFA RR73",
            "<KA1ABC> YW18FIFA RR73",
            "3140652123200000264707174620326107553140652730410160050034134266602045713140652"
        );
    }
}