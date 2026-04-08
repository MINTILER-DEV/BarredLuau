use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum CompileError {
    Io(std::io::Error),
    Parse(String),
    Scope(String),
    UnsupportedSyntax { node: String, detail: String },
    Serialize(String),
    Deserialize(String),
    Encode(String),
    Decode(String),
    Config(String),
    Integrity(String),
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Parse(message) => write!(f, "Parse error: {message}"),
            Self::Scope(message) => write!(f, "Scope analysis error: {message}"),
            Self::UnsupportedSyntax { node, detail } => {
                write!(f, "Unsupported syntax `{node}`: {detail}")
            }
            Self::Serialize(message) => write!(f, "Serialization error: {message}"),
            Self::Deserialize(message) => write!(f, "Deserialization error: {message}"),
            Self::Encode(message) => write!(f, "Encoding error: {message}"),
            Self::Decode(message) => write!(f, "Decoding error: {message}"),
            Self::Config(message) => write!(f, "Configuration error: {message}"),
            Self::Integrity(message) => write!(f, "Integrity error: {message}"),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<std::io::Error> for CompileError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
