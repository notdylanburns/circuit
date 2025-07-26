use std::iter::Peekable;

use super::file::FileRef;
use super::error::CktError;

macro_rules! classifier {
    ($name:ident { $($l:literal $i:ident),*$(,)? }) => {
        #[derive(Clone, Debug, PartialEq)]
        pub enum $name {
            $($i),*
        }

        impl $name {
            pub fn classify(string: &Vec<u8>) -> Option<Self> {
                match core::str::from_utf8(&string[..]).unwrap() {
                    $(
                        $l => Some(Self::$i),
                    )*
                    _ => None,
                }
            }

            fn to_generic_str(&self) -> &str {
                match self {
                    $(
                        Self::$i => concat!("'", $l, "'"),
                    )*
                }
            }
        }
    };
}

classifier!(PunctuationType {
    "(" LParen,
    ")" RParen,
    "[" LSquare,
    "]" RSquare,
    "{" LCurly,
    "}" RCurly,
    "<" LAngle,
    ">" RAngle,
    "." Period,
    "," Comma,
    ":" Colon,
    ";" SemiColon,
});


classifier!(KeywordType {
    "as"    As,
    "circ"  Circ,
    "let"   Let,
    "rep"   Rep,
    "use"   Use,
});


#[derive(Clone, Debug, PartialEq)]
pub enum TokenType {
    Number(usize),
    Keyword(KeywordType),
    Ident(String),
    Punctuation(PunctuationType),
}

impl TokenType {
    pub fn classify(string: &Vec<u8>) -> Option<Self> {
        assert_ne!(string.len(), 0);

        match PunctuationType::classify(&string) {
            Some(t) => return Some(Self::Punctuation(t)),
            None => (),
        };

        match KeywordType::classify(string) {
            Some(t) => return Some(Self::Keyword(t)),
            None => (),
        }

        if Self::is_num(string[0]) {
            match Self::try_parse_int(&string) {
                Some(n) => return Some(Self::Number(n)),
                None => return None,
            }
        }

        match Self::try_parse_ident(string) {
            Some(s) => return Some(Self::Ident(s)),
            None => ()
        };

        return None;
    }

    fn try_parse_int(string: &Vec<u8>) -> Option<usize> {
        string.iter().try_fold(
            0usize,
            |a, c| 
                Self::is_num(*c)
                    .then_some(a * 10 + (c - b'0') as usize)
                    .or(None)
        )
    }

    fn is_alpha(c: u8) -> bool {
        (b'a' <= c && c <= b'z') || (b'A' <= c && c <= b'Z')
    }

    fn is_num(c: u8) -> bool {
        b'0' <= c && c <= b'9'
    }

    fn try_parse_ident(string: &Vec<u8>) -> Option<String> {
        if !Self::is_alpha(string[0]) && string[0] != b'_' {
            return None;
        };

        if !string.iter().skip(1).all(|c| Self::is_alpha(*c) || Self::is_num(*c)) {
            return None;
        };

        String::from_utf8(string.clone()).map_or_else(|_| None, |s| Some(s))
    }

    pub fn to_generic_str(&self) -> &str {
        match self {
            TokenType::Ident(_) => "identifier",
            TokenType::Keyword(kt) => kt.to_generic_str(),
            TokenType::Number(_) => "number",
            TokenType::Punctuation(pt) => pt.to_generic_str(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub loc: usize,
    pub span: usize,
}

impl Token {
    pub fn new(token: &Vec<u8>, loc: usize, span: usize) -> Option<Self> {
        let token_type = TokenType::classify(token)?;

        Some(Self {
            token_type,
            loc,
            span,
        })
    }

    pub fn is_whitespace(c: u8) -> bool {
        match c {
            b' '|b'\n' => true,
            _ => false,
        }
    }

    pub fn is_token_break(c: u8) -> bool {
        match PunctuationType::classify(&vec![c]) {
            Some(_) => true,
            None => false,
        }
    }
}

#[derive(Debug)]
pub struct TokenStream {
    pub tokens: Peekable<std::vec::IntoIter<Token>>,
    pub source: FileRef,
}

impl TokenStream {
    pub fn next(&mut self) -> Result<Token, CktError> {
        self.tokens.next().ok_or_else(|| CktError::UnexpectedEOF)
    }

    pub fn peek(&mut self) -> Result<&Token, CktError> {
        self.tokens.peek().ok_or_else(|| CktError::UnexpectedEOF)
    }
}