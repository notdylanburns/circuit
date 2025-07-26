use crate::diagnostics::{diagnostic, Diagnostic};
use crate::loader::ModuleId;
use crate::util::{Interner, Pos, Position};
use crate::Diagnostics;
use std::iter::{Iterator, Peekable};
use std::num::IntErrorKind;
use std::str::CharIndices;

macro_rules! count {
    ($($items:tt),*) => {
        <[_]>::len(&[$($items),+])
    };
}

macro_rules! string_enum {
    (
        $(#[$m:meta])*
        $v:vis enum $name:ident {
            $($e:ident($l:literal)),*
            $(,)?
        }
    ) => {
        $(#[$m])*
        $v enum $name {
            $($e),*
        }

        #[allow(dead_code)]
        impl $name {
            const MEMBERS: [&'static str; count!($($l),*)] = [$($l),*];

            fn is_member(v: &str) -> bool {
                match v {
                    $($l => true,)*
                    _ => false,
                }
            }

            pub fn value(&self) -> &'static str {
                match self {
                    $($name::$e => $l,)*
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = ();

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    $(
                        $l => Ok(Self::$e),
                    )*
                    _ => Err(()),
                }
            }
        }
    };
}

string_enum! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub(crate) enum KeywordType {
        Const("const"),
        Circ("circ"),
        Enum("enum"),
        Assert("assert"),
        For("for"),
        In("in"),
        With("with"),
        Import("import"),
        If("if"),
        Else("else"),
    }
}

string_enum! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub(crate) enum PunctuationType {
        LParen("("),
        RParen(")"),
        LSquare("["),
        RSquare("]"),
        LCurly("{"),
        RCurly("}"),
        Equal("="),
        DEqual("=="),
        ExclamationEqual("!="),
        LessThan("<"),
        DLessThan("<<"),
        LessThanEqual("<="),
        GreaterThan(">"),
        DGreaterThan(">>"),
        GreaterThanEqual(">="),
        LArrow("<-"),
        RArrow("->"),
        Period("."),
        Comma(","),
        Pipe("|"),
        DPipe("||"),
        Range(".."),
        Colon(":"),
        DColon("::"),
        Semicolon(";"),
        Plus("+"),
        Minus("-"),
        Star("*"),
        Slash("/"),
        Percent("%"),
        Caret("^"),
        DCaret("^^"),
        Ampersand("&"),
        DAmpersand("&&"),
        Exclamation("!"),
        Tilde("~"),
        Dollar("$"),
    }
}

impl PunctuationType {
    fn could_be_member(c: char) -> bool {
        Self::MEMBERS.iter().any(|s| s.starts_with([c]))
    }

    fn possible_extra_chars(s: &str, c: char) -> Option<usize> {
        // `s` is the string up to the previously seen character
        // `c` is the current character

        Self::MEMBERS
            .iter()
            .filter_map(|pt| {
                pt.starts_with(s)
                    .then(|| pt[s.len()..].starts_with([c]))?
                    .then(|| pt.len() - s.len())
            })
            .max()
    }
}

pub type IdentId = usize;

#[derive(Debug, Clone, Eq, Hash)]
pub(crate) enum TokenType {
    Ident(IdentId),
    Integer(isize),
    Keyword(KeywordType),
    Punctuation(PunctuationType),
}

impl PartialEq<TokenType> for TokenType {
    fn eq(&self, other: &TokenType) -> bool {
        match (self, other) {
            (Self::Ident(_), Self::Ident(_)) => true,
            (Self::Integer(_), Self::Integer(_)) => true,
            (Self::Keyword(a), Self::Keyword(b)) => a == b,
            (Self::Punctuation(a), Self::Punctuation(b)) => a == b,
            _ => false,
        }
    }
}

impl TokenType {
    fn is_ident_first_char(c: char) -> bool {
        Self::is_ident_char(c) && !c.is_numeric()
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric()
            || match c {
                '_' | '\'' => true,
                _ => false,
            }
    }

    fn is_ident(v: &str) -> bool {
        let mut chars = v.chars();
        chars.next().is_some_and(Self::is_ident_first_char) && chars.all(Self::is_ident_char)
    }

    fn parse_integer(v: &str) -> Result<isize, TokeniserError> {
        let (base, num_str) = match v.split_once('#') {
            Some((base_str, num_str)) => (
                u32::from_str_radix(base_str, 10).map_err(|e| {
                    TokeniserError::InvalidNumericLiteral(v.to_string(), e.kind().clone())
                })?,
                num_str,
            ),
            None => (10, v),
        };

        if 2 > base || base > 36 {
            return Err(TokeniserError::InvalidNumericBase(v.to_string()));
        }

        u32::from_str_radix(num_str, base)
            .map(|n| n as isize)
            .map_err(|e| TokeniserError::InvalidNumericLiteral(v.to_string(), e.kind().clone()))
    }

    fn from_str(value: &str, interner: &mut Interner<String>) -> Result<Self, TokeniserError> {
        if let Ok(kw) = KeywordType::try_from(value) {
            return Ok(Self::Keyword(kw));
        }

        if let Ok(punct) = PunctuationType::try_from(value) {
            return Ok(Self::Punctuation(punct));
        }

        if value.starts_with(|c: char| c.is_numeric()) {
            return Self::parse_integer(value).map(Self::Integer);
        }

        if Self::is_ident(value) {
            return Ok(Self::Ident(interner.intern_ref(value)));
        }

        Err(TokeniserError::InvalidToken(value.to_string()))
    }
}

macro_rules! ident {
    () => {
        $crate::tokeniser::TokenType::Ident(0)
    };
}

macro_rules! integer {
    () => {
        $crate::tokeniser::TokenType::Integer(0)
    };
}

macro_rules! keyword {
    ($variant:ident) => {
        $crate::tokeniser::TokenType::Keyword($crate::tokeniser::KeywordType::$variant)
    };
}

macro_rules! punctuation {
    ($variant:ident) => {
        $crate::tokeniser::TokenType::Punctuation($crate::tokeniser::PunctuationType::$variant)
    };
}

#[derive(Debug)]
pub enum TokeniserError {
    InvalidToken(String),
    InvalidNumericLiteral(String, IntErrorKind),
    InvalidNumericBase(String),
}

impl std::fmt::Display for TokeniserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidToken(s) => write!(f, "invalid token: {}", s),
            Self::InvalidNumericLiteral(s, e) => write!(
                f,
                "{}: {}",
                match e {
                    IntErrorKind::Zero => unreachable!(),
                    IntErrorKind::Empty | IntErrorKind::InvalidDigit => "invalid numeric literal",
                    IntErrorKind::PosOverflow => "numeric literal too large",
                    IntErrorKind::NegOverflow => "negative literals are not supported",
                    _ => todo!("handle other IntErrorKind variants"),
                },
                s
            ),
            Self::InvalidNumericBase(s) => write!(f, "invalid base: {}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pos: Pos,
    token: TokenType,
}

impl Token {
    fn new(
        module_id: ModuleId,
        line: usize,
        col: usize,
        start: usize,
        span: usize,
        token: TokenType,
    ) -> Self {
        Self {
            pos: Pos::new(module_id, line, col, start, span),
            token,
        }
    }

    pub fn token_type(&self) -> &TokenType {
        &self.token
    }

    pub fn value(&self) -> &str {
        match &self.token {
            TokenType::Ident(_) => "identifier",
            TokenType::Integer(_) => "integer literal",
            TokenType::Keyword(kt) => kt.value(),
            TokenType::Punctuation(pt) => pt.value(),
        }
    }
}

impl Position for Token {
    fn pos(&self) -> Pos {
        self.pos
    }
}

pub struct TokenStream<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl<'a> Iterator for TokenStream<'a> {
    type Item = &'a Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokens.get(self.index).map(|token| {
            self.index += 1;
            token
        })
    }
}

pub struct TokeniserResult {
    pub module_id: ModuleId,
    pub tokens: Vec<Token>,
    pub diagnostics: Diagnostics,
}

impl TokeniserResult {
    pub fn tokens(&self) -> TokenStream {
        TokenStream {
            tokens: &self.tokens,
            index: 0,
        }
    }
}

pub struct Tokeniser<'i> {
    module_id: ModuleId,
    input: &'i str,
    stream: Peekable<CharIndices<'i>>,
    line: usize,
    linestart: usize,
    interner: &'i mut Interner<String>,
}

impl<'i> Tokeniser<'i> {
    pub fn new(module_id: ModuleId, input: &'i str, interner: &'i mut Interner<String>) -> Self {
        Self {
            module_id,
            input,
            stream: input.char_indices().peekable(),
            line: 0,
            linestart: 0,
            interner,
        }
    }

    pub fn tokenise(mut self) -> Result<TokeniserResult, Diagnostics> {
        let mut tokens = Vec::new();
        let mut diagnostics = Diagnostics::new();

        loop {
            match self.parse_token() {
                Ok(Some(token)) => {
                    tokens.push(token);
                }
                Ok(None) => break,
                Err(e) => diagnostics.push(e),
            };
        }

        if diagnostics.has_errors() {
            return Err(diagnostics);
        } else {
            Ok(TokeniserResult {
                module_id: self.module_id,
                tokens,
                diagnostics,
            })
        }
    }

    fn newline(&mut self, linestart: usize) {
        self.line += 1;
        self.linestart = linestart + 1;
    }

    fn token_range(&self, start: usize, end: usize) -> (usize, usize, usize, usize) {
        (self.line, start - self.linestart, start, end)
    }

    fn final_token(&self, start: usize) -> (usize, usize, usize, usize) {
        (self.line, start - self.linestart, start, self.input.len())
    }

    fn handle_whitespace(
        &mut self,
        c: char,
        start: usize,
        index: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let line = self.line;
        let linestart = self.linestart;
        if c == '\n' {
            self.newline(index)
        }

        self.stream.next();
        if start == index {
            return None;
        }

        return Some((line, start - linestart, start, index));
    }

    fn handle_multi_char_punctuation(
        &mut self,
        start: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let (i, c) = match self.stream.peek() {
            Some((i, c)) => (*i, *c),
            None => unreachable!(),
        };

        let mut peeked_i = i;
        let mut peeked_c = c;
        while let Some(x) =
            PunctuationType::possible_extra_chars(&self.input[start..peeked_i], peeked_c)
        {
            self.stream.next();

            if x == 0 {
                return Some(self.token_range(start, peeked_i));
            }

            let Some((i, c)) = self.stream.peek() else {
                return Some(self.final_token(start));
            };
            let (&i, &c) = (i, c);

            if c.is_whitespace() {
                return Some(self.token_range(start, i));
            }
            peeked_i = i;
            peeked_c = c;
        }

        return Some(self.token_range(start, peeked_i));
    }

    fn get_next_token_range(&mut self) -> Option<(usize, usize, usize, usize)> {
        let mut start = None;
        while let Some((i, c)) = self.stream.peek() {
            let (&i, &c) = (i, c);
            let some_start = *start.get_or_insert(i);

            if c.is_whitespace() {
                let token_range = self.handle_whitespace(c, some_start, i);

                if token_range.is_some() {
                    return token_range;
                }

                start = None;
                continue;
            }

            if !PunctuationType::could_be_member(c) {
                self.stream.next();
                continue;
            }

            if i != some_start {
                return Some(self.token_range(some_start, i));
            }

            return self.handle_multi_char_punctuation(some_start);
        }

        start.map(|s| self.final_token(s))
    }

    fn parse_token(&mut self) -> Result<Option<Token>, Diagnostic> {
        let Some((line, col, start, end)) = self.get_next_token_range() else {
            return Ok(None);
        };

        TokenType::from_str(&self.input[start..end], &mut self.interner)
            .map(|t| Some(Token::new(self.module_id, line, col, start, end - start, t)))
            .map_err(|e| {
                diagnostic!(
                    Error,
                    TokeniserError::InvalidToken(e.to_string()),
                    pos = Pos::new(self.module_id, line, col, start, end - start),
                )
            })
    }
}

pub(crate) use {ident, integer, keyword, punctuation};
