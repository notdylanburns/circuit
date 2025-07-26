use std::io;
use super::file::FileRef;
use super::token::{Token, TokenType};

macro_rules! string_from {
    ($e:expr) => {
        String::from_utf8($e).unwrap_or_else(|e| panic!("An error occurred: {e}"))
    };
    ($e:expr, $error:expr) => {
        String::from_utf8($e).unwrap_or_else(|e| panic!("While displaying {}\nAn error occurred: {e}", $error))
    };
}

#[derive(Debug)]
pub enum CktError {
    AtLocation(Box<Self>, FileRef, usize, usize),   // File, Location, Span
    ExpectedToken(Vec<TokenType>),                  // Token options
    InvalidToken(Vec<u8>),                          // Value
    IOError(&'static str, String, io::Error),       // Action, Filepath, Error
    Received(Box<Self>, TokenType),                 // Error, Received
    ReceivedValue(Box<Self>, Vec<u8>),              // Error, Received
    UnexpectedEOF,
    UnexpectedToken((u32, &'static str), Token),
}

impl CktError {
    pub fn at_location(self, file: FileRef, loc: usize, span: usize) -> Self {
        Self::AtLocation(Box::new(self), file, loc, span)
    }

    pub fn received(self, received: TokenType) -> Self {
        Self::Received(Box::new(self), received)
    }

    pub fn received_value(self, received: Vec<u8>) -> Self {
        Self::ReceivedValue(Box::new(self), received)
    }
}

impl std::fmt::Display for CktError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtLocation(error, file, loc, span) => {
                let mut fileref = file.borrow_mut();
                let (line, col) = fileref.get_location(*loc);
                let line_bytes = fileref.read_line(line - 1)
                    .unwrap_or_else(|e| panic!("While displaying {error}\nAn error occurred: {e}"));

                let line_string = string_from!(line_bytes, error);
                write!(
                    f,
                    "{}:{line}:{col}: {error}\n\n{line_string}{:width$}{:^>span$}",
                    fileref.path,
                    "", "",
                    width = (col - 1), span = span
                )
            },
            Self::ExpectedToken(opts) => {
                if opts.len() > 1 {
                    write!(f, "expected one of {}", opts.iter().map(|o| o.to_generic_str()).collect::<Vec<&str>>().join(", "))
                } else if opts.len() == 1 {
                    write!(f, "expected {}", opts[0].to_generic_str())
                } else {
                    unreachable!()
                }
            },
            Self::InvalidToken(v) => write!(f, "invalid token: {}", string_from!(v.to_vec())),
            Self::IOError(a, p, e) => write!(f, "{} {}: {}", a, p, e),
            Self::Received(error, r) => write!(f, "{error}, received {}", r.to_generic_str()),
            Self::ReceivedValue(error, v) => write!(f, "{error}, received {}", string_from!(v.to_vec())),
            Self::UnexpectedEOF => write!(f, "unexpected EOF"),
            Self::UnexpectedToken(l, t) => write!(f, "({l:?}) unexpected token: {t:?}"),
        }
    }
}