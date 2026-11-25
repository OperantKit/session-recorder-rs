use std::fmt;
use std::io;

/// What can go wrong while writing or reading an OKL v1 log.
#[derive(Debug)]
#[non_exhaustive]
pub enum OklError {
    /// The header is malformed, incomplete, missing or written twice.
    Header(String),
    /// A body line, or a value to be written to one, does not fit its type.
    Record(String),
    /// A body line uses an event type that the codebook does not declare.
    UnknownType(String),
    /// The file could not be read or written.
    Io(io::Error),
}

impl fmt::Display for OklError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OklError::Header(m) => write!(f, "OKL v1 header: {m}"),
            OklError::Record(m) => write!(f, "OKL v1 record: {m}"),
            OklError::UnknownType(t) => {
                write!(f, "unknown event type {t:?} (not declared in the codebook)")
            }
            OklError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OklError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OklError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for OklError {
    fn from(e: io::Error) -> Self {
        OklError::Io(e)
    }
}
