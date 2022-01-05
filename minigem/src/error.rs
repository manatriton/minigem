use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    BadHost,
    BadScheme,
    InvalidUtf8,
    Io(io::Error),
    ParseUrl(url::ParseError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::BadHost => write!(f, "bad host"),
            Error::BadScheme => write!(f, "bad scheme"),
            Error::InvalidUtf8 => write!(f, "invalid utf-8"),
            Error::Io(ref err) => err.fmt(f),
            Error::ParseUrl(ref err) => err.fmt(f),
        }
    }
}

impl error::Error for Error {}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Self::ParseUrl(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
