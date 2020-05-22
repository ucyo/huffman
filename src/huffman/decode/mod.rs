pub mod iterator;
pub mod one;
pub mod reader;
pub mod symboltable;
pub mod writer;

#[derive(Debug)]
enum DecoderError {
    AllOutputAlreadyWritten,
    BufferOverflow,
    IO(std::io::Error),
}

use std::fmt;
impl std::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DecoderError::AllOutputAlreadyWritten => write!(f, "All ouput is already written"),
            DecoderError::BufferOverflow => write!(f, "The inner buffer element is overflowing"),
            DecoderError::IO(ref err) => err.fmt(f),
        }
    }
}

use std::error::Error;
impl Error for DecoderError {}

impl std::convert::From<std::io::Error> for DecoderError {
    fn from(err: std::io::Error) -> Self {
        DecoderError::IO(err)
    }
}

impl std::convert::Into<std::io::Error> for DecoderError {
    fn into(self) -> std::io::Error {
        match self {
            DecoderError::IO(err) => err,
            _ => std::io::Error::new(std::io::ErrorKind::Other, self),
        }
    }
}
