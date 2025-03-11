use crate::constants::{CHANNEL_SYMBOLS_COUNT, FT8_COSTAS};

use super::{message_parse_error::MessageParseError, Message};

impl TryFrom<&[u8]> for Message {
    type Error = MessageParseError;

    fn try_from(channel_symbols: &[u8]) -> Result<Self, Self::Error> {
        if channel_symbols.len() != CHANNEL_SYMBOLS_COUNT {
            return Err(MessageParseError::InvalidSymbolsLength);
        }

        // must start with costas
        if channel_symbols[0..FT8_COSTAS.len()] != FT8_COSTAS {
            return Err(MessageParseError::InvalidSymbols);
        }

        // must end with costas
        if channel_symbols[channel_symbols.len() - FT8_COSTAS.len()..] != FT8_COSTAS {
            return Err(MessageParseError::InvalidSymbols);
        }

        todo!()
    }
}
