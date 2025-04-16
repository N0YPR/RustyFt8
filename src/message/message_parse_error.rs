use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum MessageParseError {
    /// String contains invalid character
    #[snafu(display("message_string contains an invalid character"))]
    InvalidChar,

    /// String could not be parsed as a valid message
    #[snafu(display("message_string could not be parsed as a valid message"))]
    InvalidMessage,

    /// Empty String
    #[snafu(display("message_string empty"))]
    EmptyString,
}
