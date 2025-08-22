use crate::diagnostics::{diagnostic, Diagnostic, Diagnostics};
use crate::loader::ModuleId;
use crate::tokeniser::{
    ident, keyword, punctuation, KeywordType as kt, PunctuationType as pt, Token, TokenStream,
    TokenType as tt, TokenType,
};
use crate::util::{ChainMap, Pos, Position};
use core::iter::Peekable;

use crate::ast::{
    Associativity, ConnectionDirection, ConstExprOpType, Node, NodeType, PinDirection,
    PinExprOpType, AST,
};

#[derive(Debug)]
pub enum ParserError {
    UnexpectedEof,
    Expected(String, String),
    ExprError(ExprErrorKind),
}

#[derive(Debug)]
pub enum ExprErrorKind {
    ExpectedBinaryOperator(String),
    UnmatchedClosingParenthesis,
    UnmatchedOpeningParenthesis,
}

impl core::fmt::Display for ParserError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of file"),
            Self::Expected(expected, got) => write!(f, "expected {}, got '{}'", expected, got),
            Self::ExprError(kind) => match kind {
                ExprErrorKind::ExpectedBinaryOperator(op) => {
                    write!(f, "expected binary operator, got unary '{op}'")
                }
                ExprErrorKind::UnmatchedClosingParenthesis => write!(f, "unmatched ')'"),
                ExprErrorKind::UnmatchedOpeningParenthesis => write!(f, "'(' was not closed"),
            },
        }
    }
}

#[derive(Debug)]
enum ParseFailKind {
    Recover,
    Critical(Diagnostic),
}

impl From<Diagnostic> for ParseFailKind {
    fn from(error: Diagnostic) -> Self {
        ParseFailKind::Critical(error)
    }
}

enum Caught<T> {
    Ok(T),
    Recover,
}

impl<T> Caught<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Caught::Ok(value) => Some(value),
            Caught::Recover => None,
        }
    }
}

impl<T> Into<Option<T>> for Caught<T> {
    fn into(self) -> Option<T> {
        self.into_option()
    }
}

#[allow(unused)]
trait AlwaysResultTrait<T> {
    fn critical(error: Diagnostic) -> Self;
    fn recover() -> Self;
    fn is_critical(&self) -> bool;
    fn is_recoverable(&self) -> bool;
    fn catch(self) -> Result<Caught<T>, Diagnostic>;
    fn wrap(self) -> ParseResult<T>;
}

trait ParseResultTrait<T>: AlwaysResultTrait<Option<T>> {
    fn some(value: T) -> Self;
    fn none() -> Self;
}

type AlwaysResult<T> = Result<T, ParseFailKind>;
type ParseResult<T> = Result<Option<T>, ParseFailKind>;

impl<T> AlwaysResultTrait<T> for AlwaysResult<T> {
    fn critical(error: Diagnostic) -> Self {
        Err(ParseFailKind::Critical(error))
    }

    fn recover() -> Self {
        Err(ParseFailKind::Recover)
    }

    fn is_critical(&self) -> bool {
        matches!(self, Err(ParseFailKind::Critical(_)))
    }

    fn is_recoverable(&self) -> bool {
        matches!(self, Err(ParseFailKind::Recover))
    }

    fn catch(self) -> Result<Caught<T>, Diagnostic> {
        match self {
            Ok(value) => Ok(Caught::Ok(value)),
            Err(ParseFailKind::Critical(error)) => Err(error),
            Err(ParseFailKind::Recover) => Ok(Caught::Recover),
        }
    }

    fn wrap(self) -> ParseResult<T> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(ParseFailKind::Critical(error)) => ParseResult::critical(error),
            Err(ParseFailKind::Recover) => ParseResult::recover(),
        }
    }
}

impl<T> ParseResultTrait<T> for ParseResult<T> {
    fn some(value: T) -> Self {
        Ok(Some(value))
    }

    fn none() -> Self {
        Ok(None)
    }
}

#[derive(Debug)]
struct RecoverParams {
    allow_eof: bool,
    consume_on_success: bool,
}

impl Default for RecoverParams {
    fn default() -> Self {
        Self {
            allow_eof: false,
            consume_on_success: true,
        }
    }
}

macro_rules! recover {
    () => {{
        return Err(ParseFailKind::Recover);
    }};
    (
        $self:expr, $expr:expr, $sentinels:tt
    ) => {{
        recover!($self, $expr, $sentinels,)
    }};
    (
        $self:expr,
        $expr:expr,
        $sentinels:tt,
        $(
            $cfg:ident = $v:literal
         ),*
        $(,)?
    ) => {{
        let params = recover!(@buildparams RecoverParams::default(), $($cfg = $v,)*);

        let sentinels = $self.push_sentinels(&$sentinels);
        let mut result = (|| $expr)();

        let should_consume = params.consume_on_success || matches!(result, Err(ParseFailKind::Recover));

        if should_consume {
            let next_tk = if params.allow_eof {
                $self.consume_until_allow_eof(&sentinels)
            } else {
                Some($self.consume_until(&sentinels)?)
            };

            result = match result {
                Err(ParseFailKind::Recover) => {
                    if let Some(tk) = next_tk {
                        let depth = $self
                            .sentinels
                            .get(&tk.token_type().clone())
                            .unwrap_or_else(|| unreachable!());

                        if *depth == $self.sentinels.chain_len() {
                            Ok(None)
                        } else {
                            Err(ParseFailKind::Recover)
                        }
                    } else {
                        Ok(None)
                    }
                }
                result => result,
            };
        };

        $self.sentinels.pop_chain();
        result
    }};
    (@buildparams $p:expr$(,)?) => {
        $p
    };
    (@buildparams $p:expr, $cfg:ident = $v:literal) => {
        recover!(@buildparams, $p, $cfg = $v,)
    };
    (@buildparams $p:expr, $cfg:ident = $v:literal, $($extra:tt)*) => {{
        let mut p = $p;
        p.$cfg = $v;

        recover!(@buildparams p, $($extra)*)
    }};
}

pub struct ParserResult {
    pub ast: AST,
    pub diagnostics: Diagnostics,
}

impl ParserResult {
    fn new(ast: AST, diagnostics: Diagnostics) -> Self {
        Self { ast, diagnostics }
    }
}

pub struct Parser<'a> {
    module: ModuleId,
    tokens: Peekable<TokenStream<'a>>,
    diagnostics: Diagnostics,
    sentinels: ChainMap<TokenType, usize>,
    last_token_pos: Pos,
}

impl<'a> Parser<'a> {
    pub fn new(module: ModuleId, tokens: TokenStream<'a>) -> Self {
        Self {
            module,
            tokens: tokens.peekable(),
            diagnostics: Diagnostics::new(),
            sentinels: ChainMap::new(),
            last_token_pos: Pos::None,
        }
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    fn push_sentinels(&mut self, sentinels: &[TokenType]) -> Vec<TokenType> {
        let depth = self.sentinels.chain_len() + 1;
        let map = sentinels.iter().cloned().map(|tt| (tt, depth));

        self.sentinels.push_chain(map);

        self.sentinels.keys().cloned().collect()
    }

    fn parse_path(&mut self) -> AlwaysResult<Node> {
        let start_token = self.expect_token_is(ident!())?;

        let mut path = Vec::new();
        path.push(start_token);

        loop {
            match self.peek_next_token()?.token_type() {
                tt::Punctuation(pt::Colon) => {
                    self.consume_token()?;
                    let ident = self.expect_token_is(ident!())?;
                    path.push(ident);
                }
                _ => break,
            }
        }

        Ok(NodeBuilder::path_from_tokens(path))
    }

    pub fn parse(mut self) -> Result<ParserResult, Diagnostics> {
        let mut ast = AST::new(self.module);

        loop {
            if !self.has_next_token() {
                break;
            }

            match self.parse_item() {
                Ok(Some(item)) => ast.add_node(item),
                Ok(None) => (),
                Err(ParseFailKind::Critical(e)) => self.diagnostics.push(e),
                Err(ParseFailKind::Recover) => unreachable!("recovery propogated to parse"),
            }
        }

        if !self.diagnostics.has_errors() {
            Ok(ParserResult::new(ast, self.diagnostics))
        } else {
            Err(self.diagnostics)
        }
    }

    fn parse_item(&mut self) -> ParseResult<Node> {
        const STARTING_TOKENS: [TokenType; 7] = [
            keyword!(Assert),
            keyword!(Circ),
            keyword!(Const),
            keyword!(Enum),
            keyword!(Import),
            keyword!(If),
            keyword!(Use),
        ];

        recover!(
            self,
            {
                let tk = self.peek_next_token()?;

                match tk.token_type() {
                    tt::Keyword(kt) => match kt {
                        kt::Assert => self.parse_assert(),
                        kt::Circ => self.parse_circ(),
                        kt::Const => self.parse_const(),
                        kt::Enum => self.parse_enum(),
                        kt::If => self.parse_if(&mut Self::parse_item),
                        kt::Import => self.parse_import(),
                        kt::Use => self.parse_use(),
                        _ => {
                            self.add_error(self.expected_error(&STARTING_TOKENS, &tk));
                            recover!()
                        }
                    },
                    _ => {
                        self.add_error(self.expected_error(&STARTING_TOKENS, &tk));
                        recover!()
                    }
                }
            },
            STARTING_TOKENS,
            allow_eof = true,
            consume_on_success = false
        )
    }

    fn parse_circ(&mut self) -> ParseResult<Node> {
        let start_token = self.assert_token_is(keyword!(Circ));
        let node = NodeBuilder::new(&start_token);

        let name = recover!(
            self,
            self.expect_token_is(ident!()).wrap(),
            [punctuation!(LParen), punctuation!(LCurly)]
        )?
        .map(NodeBuilder::identifier_from_token);

        let args =
            recover!(
                self,
                {
                    let mut args = Vec::new();

                    if self.peek_next_token()?.token_type() != &punctuation!(LParen) {
                        return Ok(args).wrap();
                    }

                    self.assert_token_is(punctuation!(LParen));

                    while self.peek_next_token()?.token_type() != &punctuation!(RParen) {
                        let arg = recover!(
                            self,
                            self.parse_circ_arg(),
                            [punctuation!(Comma), punctuation!(RParen)]
                        )?;

                        match arg {
                            Some(arg) => args.push(arg),
                            _ => (),
                        }

                        let tk = self.peek_next_token()?;
                        match tk.token_type() {
                            tt::Punctuation(pt::Comma) => {
                                self.consume_token()?;
                            }
                            tt::Punctuation(pt::RParen) => {
                                break;
                            }
                            _ => {
                                self.add_error(self.expected_error(
                                    &[punctuation!(Comma), punctuation!(RParen)],
                                    &tk,
                                ));
                                recover!()
                            }
                        }
                    }

                    self.assert_token_is(punctuation!(RParen));

                    Ok(args).wrap()
                },
                [punctuation!(LCurly)]
            )?;
        self.expect_token_is(punctuation!(LCurly))?;

        let mut children = Vec::new();

        loop {
            match self.peek_next_token()?.token_type() {
                tt::Punctuation(pt::RCurly) => {
                    break;
                }
                _ => (),
            }

            let statement = recover!(
                self,
                self.parse_statement(),
                [
                    punctuation!(RCurly),
                    keyword!(Assert),
                    keyword!(Const),
                    keyword!(For),
                    keyword!(With)
                ],
                consume_on_success = false
            )?;

            match statement {
                Some(child) => children.push(child),
                None => continue,
            }
        }

        let end_token = self.assert_token_is(punctuation!(RCurly));
        let end_token = self.consume_optional_semicolon().unwrap_or(end_token);

        match name.zip(args) {
            Some((name, args)) => {
                ParseResult::some(node.circ(name, args, children).build(&end_token))
            }
            _ => ParseResult::none(),
        }
    }

    fn parse_circ_arg(&mut self) -> ParseResult<Node> {
        let result = recover!(
            self,
            {
                let r#type = self.parse_path()?;
                let name = NodeBuilder::identifier_from_token(self.expect_token_is(ident!())?);

                Ok((r#type, name)).wrap()
            },
            [punctuation!(Equal)]
        )?;

        let default = match self.peek_next_token()?.token_type() {
            tt::Punctuation(pt::Equal) => {
                self.consume_token()?;
                Some(self.parse_constexpr()?)
            }
            _ => None,
        };

        let Some((r#type, name)) = result else {
            return ParseResult::none();
        };

        let node = NodeBuilder::new(&r#type);
        let end_pos = default.as_ref().unwrap_or(&name).pos();

        ParseResult::some(node.circ_arg(r#type, name, default).build(&end_pos))
    }

    fn parse_const(&mut self) -> ParseResult<Node> {
        let start_token = self.assert_token_is(keyword!(Const));

        let result = recover!(
            self,
            {
                let result = recover!(
                    self,
                    {
                        let r#type = self.parse_path()?;
                        let name =
                            NodeBuilder::identifier_from_token(self.expect_token_is(ident!())?);

                        self.expect_next_token_is(punctuation!(Equal))?;

                        Ok((r#type, name)).wrap()
                    },
                    [punctuation!(Equal)]
                )?;

                self.assert_token_is(punctuation!(Equal));

                let value = self.parse_constexpr()?;

                self.expect_next_token_is(punctuation!(Semicolon))?;

                Ok((start_token, result, value)).wrap()
            },
            [punctuation!(Semicolon)]
        )?;

        let Some((start_token, Some((r#type, name)), value)) = result else {
            return ParseResult::none();
        };

        let node = NodeBuilder::new(&start_token);
        let end_token = self.assert_token_is(punctuation!(Semicolon));

        ParseResult::some(node.const_decl(r#type, name, value).build(&end_token))
    }

    fn parse_enum(&mut self) -> ParseResult<Node> {
        let start_token = self.assert_token_is(keyword!(Enum));

        let name = recover!(
            self,
            {
                let name = self.expect_token_is(ident!())?;
                self.expect_next_token_is(punctuation!(LCurly))?;
                Ok(name).wrap()
            },
            [punctuation!(LCurly)]
        )?
        .map(NodeBuilder::identifier_from_token);

        self.assert_token_is(punctuation!(LCurly));

        let mut variants = Vec::new();

        while self.peek_next_token()?.token_type() != &punctuation!(RCurly) {
            let variant = recover!(
                self,
                self.expect_token_is(ident!()).wrap(),
                [punctuation!(Comma), punctuation!(RCurly)]
            )?
            .map(NodeBuilder::identifier_from_token);

            match variant {
                Some(variant) => variants.push(variant),
                None => continue,
            }

            let tk = self.peek_next_token()?;
            match tk.token_type() {
                tt::Punctuation(pt::Comma) => {
                    self.consume_token()?;
                    continue;
                }
                tt::Punctuation(pt::RCurly) => {
                    break;
                }
                _ => {
                    self.add_error(self.expected_error(&[punctuation!(Comma)], &tk));
                    recover!()
                }
            }
        }

        let end_token = self.assert_token_is(punctuation!(RCurly));
        let end_token = self.consume_optional_semicolon().unwrap_or(end_token);

        let Some(name) = name else {
            return ParseResult::none();
        };

        let node = NodeBuilder::new(&start_token);
        ParseResult::some(node.r#enum(name, variants).build(&end_token))
    }

    fn parse_import(&mut self) -> ParseResult<Node> {
        let start = self.assert_token_is(keyword!(Import));
        let node = NodeBuilder::new(&start);

        let name = recover!(
            self,
            {
                let name = self.expect_token_is(ident!())?;
                self.expect_next_token_is(punctuation!(Semicolon))?;
                Ok(name).wrap()
            },
            [punctuation!(Semicolon)]
        )?
        .map(NodeBuilder::identifier_from_token);
        let end_tk = self.assert_token_is(punctuation!(Semicolon));

        let Some(name) = name else {
            return ParseResult::none();
        };

        ParseResult::some(node.import(name).build(&end_tk))
    }

    fn parse_if(
        &mut self,
        branch_parser: &mut impl FnMut(&mut Self) -> ParseResult<Node>,
    ) -> ParseResult<Node> {
        let start_token = self.assert_token_is(keyword!(If));
        let node = NodeBuilder::new(&start_token);

        let result = recover!(
            self,
            {
                let condition = recover!(
                    self,
                    {
                        let result = self.parse_constexpr()?;
                        self.expect_next_token_is(punctuation!(LCurly))?;
                        ParseResult::some(result)
                    },
                    [punctuation!(LCurly)]
                )?;

                self.assert_token_is(punctuation!(LCurly));

                let mut then = Vec::new();

                while self.peek_next_token()?.token_type() != &punctuation!(RCurly) {
                    match branch_parser(self)? {
                        Some(stmt) => then.push(stmt),
                        None => (),
                    }
                }

                ParseResult::some((condition, then))
            },
            [punctuation!(RCurly)]
        )?;

        let mut end_pos = self.assert_token_is(punctuation!(RCurly)).pos();

        let mut r#else = Vec::new();

        if self.peek_next_token()?.token_type() == &keyword!(Else) {
            self.assert_token_is(keyword!(Else));

            match self.peek_next_token()?.token_type() {
                tt::Keyword(kt::If) => match self.parse_if(branch_parser)? {
                    Some(node) => {
                        end_pos = node.pos();
                        r#else.push(node)
                    }
                    None => (),
                },
                tt::Punctuation(pt::LCurly) => {
                    recover!(
                        self,
                        {
                            self.assert_token_is(punctuation!(LCurly));
                            while self.peek_next_token()?.token_type() != &punctuation!(RCurly) {
                                match branch_parser(self)? {
                                    Some(stmt) => r#else.push(stmt),
                                    None => (),
                                }
                            }
                            Ok(Some(()))
                        },
                        [punctuation!(RCurly)]
                    )?;

                    end_pos = self.assert_token_is(punctuation!(RCurly)).pos();
                }
                _ => (),
            }
        };

        let end_pos = self
            .consume_optional_semicolon()
            .map(|tk| tk.pos())
            .unwrap_or(end_pos);

        let Some((Some(condition), then)) = result else {
            return ParseResult::none();
        };

        ParseResult::some(node.r#if(condition, then, r#else).build(&end_pos))
    }

    fn parse_use(&mut self) -> ParseResult<Node> {
        let start = self.assert_token_is(keyword!(Use));

        let result = recover!(
            self,
            {
                let path = recover!(self, self.parse_path().wrap(), [keyword!(As)])?;

                let name = match self.peek_next_token()?.token_type() {
                    tt::Keyword(kt::As) => {
                        self.consume_token()?;
                        let name = self.expect_token_is(ident!())?;
                        Some(NodeBuilder::identifier_from_token(name))
                    }
                    _ => None,
                };

                self.expect_next_token_is(punctuation!(Semicolon))?;

                ParseResult::some((path, name))
            },
            [punctuation!(Semicolon)]
        )?;

        let end_tk = self.assert_token_is(punctuation!(Semicolon));

        let Some((Some(path), name)) = result else {
            return ParseResult::none();
        };

        let node = NodeBuilder::new(&start);

        ParseResult::some(node.r#use(path, name).build(&end_tk))
    }

    fn parse_statement(&mut self) -> ParseResult<Node> {
        let tk = self.peek_next_token()?;

        let result = recover!(
            self,
            {
                match tk.token_type() {
                    tt::Punctuation(pt::Semicolon) => {
                        self.consume_token()?;
                        ParseResult::none()
                    }
                    tt::Ident(_) => self.parse_connection(),
                    tt::Punctuation(pt::LArrow | pt::RArrow) => self.parse_pindecl(),
                    tt::Keyword(kt::Assert) => self.parse_assert(),
                    tt::Keyword(kt::Const) => self.parse_const(),
                    tt::Keyword(kt::If) => self.parse_if(&mut Self::parse_statement),
                    tt::Keyword(kt::For) => todo!(),
                    tt::Keyword(kt::With) => self.parse_with(),
                    _ => {
                        self.add_error(self.expected_error(
                            &[
                                keyword!(Assert),
                                keyword!(Const),
                                keyword!(For),
                                keyword!(With),
                                keyword!(If),
                                punctuation!(LArrow),
                                punctuation!(RArrow),
                                ident!(),
                            ],
                            &tk,
                        ));
                        recover!()
                    }
                }
            },
            [punctuation!(Semicolon)],
            consume_on_success = false,
        )?;

        let Some(statement) = result else {
            return ParseResult::none();
        };

        let end_pos = self
            .consume_optional_semicolon()
            .map_or(statement.pos(), |tk| tk.pos());

        let node = NodeBuilder::new(&statement);
        ParseResult::some(node.statement(statement).build(&end_pos))
    }

    fn parse_pindecl(&mut self) -> ParseResult<Node> {
        let start_tk = self.consume_token()?;
        let node = NodeBuilder::new(&start_tk);

        let direction = match start_tk.token_type() {
            tt::Punctuation(pt::LArrow) => PinDirection::Output,
            tt::Punctuation(pt::RArrow) => PinDirection::Input,
            _ => unreachable!(),
        };

        let Some(decls) = self.parse_decl_list()? else {
            return ParseResult::none();
        };

        self.expect_next_token_is(punctuation!(Semicolon))?;

        ParseResult::some(node.pin_decls(direction, decls).build(&self.last_token_pos))
    }

    fn parse_assert(&mut self) -> ParseResult<Node> {
        let start_tk = self.assert_token_is(keyword!(Assert));
        let node = NodeBuilder::new(&start_tk);

        let expr = recover!(
            self,
            {
                let result = self.parse_constexpr()?;
                self.expect_next_token_is(punctuation!(Semicolon))?;
                Ok(Some(result))
            },
            [punctuation!(Semicolon)]
        )?;

        let end_tk = self.assert_token_is(punctuation!(Semicolon));

        let Some(expr) = expr else {
            return ParseResult::none();
        };

        ParseResult::some(node.assert(expr).build(&end_tk))
    }

    fn parse_with(&mut self) -> ParseResult<Node> {
        let start_tk = self.assert_token_is(keyword!(With));
        let node = NodeBuilder::new(&start_tk);

        let mut decls = Vec::new();

        match self.peek_next_token()?.token_type() {
            tt::Punctuation(pt::LCurly) => {
                self.assert_token_is(punctuation!(LCurly));
                while self.peek_next_token()?.token_type() != &punctuation!(RCurly) {
                    let decl = recover!(
                        self,
                        {
                            let result = self.parse_with_decl()?;
                            self.expect_next_token_one_of(&[
                                punctuation!(RCurly),
                                punctuation!(Semicolon),
                            ])?;

                            Ok(result)
                        },
                        [punctuation!(RCurly), punctuation!(Semicolon)]
                    )?;

                    if let Some(decl) = decl {
                        decls.push(decl);
                    };

                    match self.peek_next_token()?.token_type() {
                        tt::Punctuation(pt::RCurly) => break,
                        tt::Punctuation(pt::Semicolon) => self.consume_token()?,
                        _ => unreachable!(),
                    };
                }

                self.assert_token_is(punctuation!(RCurly));
            }
            _ => {
                let decl = recover!(
                    self,
                    {
                        let result = self.parse_with_decl()?;
                        self.expect_next_token_is(punctuation!(Semicolon))?;
                        Ok(result)
                    },
                    [punctuation!(Semicolon)]
                )?;

                if let Some(decl) = decl {
                    decls.push(decl);
                };
            }
        }

        ParseResult::some(node.with(decls).build(&self.last_token_pos))
    }

    fn parse_with_decl(&mut self) -> ParseResult<Node> {
        let r#type = self.parse_type()?;

        let Some(decls) = self.parse_decl_list()? else {
            return ParseResult::none();
        };

        let Some(r#type) = r#type else {
            return ParseResult::none();
        };

        let node = NodeBuilder::new(&r#type.pos());

        ParseResult::some(node.decls(r#type, decls).build(&self.last_token_pos))
    }

    fn parse_decl_list(&mut self) -> ParseResult<Vec<Node>> {
        let mut decls = Vec::new();

        loop {
            let decl = recover!(
                self,
                {
                    let name = NodeBuilder::identifier_from_token(self.expect_token_is(ident!())?);

                    if self.peek_next_token()?.token_type() != &punctuation!(LSquare) {
                        return ParseResult::some(NodeBuilder::decl_from_name(name));
                    };

                    let node = NodeBuilder::new(&name);

                    self.assert_token_is(punctuation!(LSquare));
                    let size = self.parse_constexpr()?;
                    let end_tk = self.expect_token_is(punctuation!(RSquare))?;

                    ParseResult::some(node.decl(name, size).build(&end_tk))
                },
                [punctuation!(Comma)],
                consume_on_success = false
            )?;

            if let Some(decl) = decl {
                decls.push(decl);
            }

            let tk = self.peek_next_token()?;
            match tk.token_type() {
                tt::Punctuation(pt::Comma) => self.consume_token()?,
                _ => break ParseResult::some(decls),
            };
        }
    }

    fn parse_type(&mut self) -> ParseResult<Node> {
        let name = self.parse_path()?;
        let start_pos = name.pos();

        let node = NodeBuilder::new(&start_pos);

        let mut args = Vec::new();

        if self.peek_next_token()?.token_type() != &punctuation!(LParen) {
            return ParseResult::some(node.r#type(name, args).build(&start_pos));
        }

        self.assert_token_is(punctuation!(LParen));

        while self.peek_next_token()?.token_type() != &punctuation!(RParen) {
            let arg = recover!(
                self,
                self.parse_type_arg(),
                [punctuation!(Comma), punctuation!(RParen)]
            )?;

            match arg {
                Some(arg) => args.push(arg),
                _ => (),
            }

            let tk = self.peek_next_token()?;
            match tk.token_type() {
                tt::Punctuation(pt::Comma) => {
                    self.consume_token()?;
                    continue;
                }
                tt::Punctuation(pt::RParen) => {
                    break;
                }
                _ => {
                    self.add_error(
                        self.expected_error(&[punctuation!(Comma), punctuation!(RParen)], &tk),
                    );
                    recover!()
                }
            }
        }

        ParseResult::some(
            node.r#type(name, args)
                .build(&self.assert_token_is(punctuation!(RParen))),
        )
    }

    fn parse_type_arg(&mut self) -> ParseResult<Node> {
        let tk = self.peek_next_token()?;
        let node = NodeBuilder::new(&tk);

        match tk.token_type() {
            tt::Ident(..) => {
                let ident = self.consume_token()?;
                if self.peek_next_token()?.token_type() != &punctuation!(Equal) {
                    let value = self.parse_constexpr_with_starting_ident(Some(ident))?;
                    let end_pos = value.pos();
                    return ParseResult::some(node.type_arg(None, value).build(&end_pos));
                };

                let ident = NodeBuilder::identifier_from_token(ident);
                self.assert_token_is(punctuation!(Equal));

                let value = self.parse_constexpr()?;
                let end_pos = value.pos();
                ParseResult::some(node.type_arg(Some(ident), value).build(&end_pos))
            }
            _ => {
                let value = self.parse_constexpr()?;
                let end_pos = value.pos();
                ParseResult::some(node.type_arg(None, value).build(&end_pos))
            }
        }
    }

    fn parse_connection(&mut self) -> ParseResult<Node> {
        let tk = self.peek_next_token()?;
        let node = NodeBuilder::new(&tk);

        let lhs = recover!(
            self,
            {
                let lhs = self.parse_pinexpr()?;
                self.expect_next_token_one_of(&[punctuation!(LArrow), punctuation!(RArrow)])?;
                ParseResult::some(lhs)
            },
            [punctuation!(LArrow), punctuation!(RArrow)]
        )?;

        let direction = match self.consume_token()?.token_type() {
            tt::Punctuation(pt::LArrow) => ConnectionDirection::RightToLeft,
            tt::Punctuation(pt::RArrow) => ConnectionDirection::LeftToRight,
            // tt::Punctuation("<>") => ConnectionDirection::Bidirectional,
            _ => unreachable!("parse_connection should only be called for LArrow or RArrow"),
        };

        let rhs = self.parse_pinexpr()?;
        let end_pos = rhs.pos();

        let Some(lhs) = lhs else {
            return ParseResult::none();
        };

        // Not that great but should work
        self.expect_next_token_is(punctuation!(Semicolon));

        ParseResult::some(node.connection(lhs, direction, rhs).build(&end_pos))
    }

    fn parse_pinexpr(&mut self) -> AlwaysResult<Node> {
        enum OpItem {
            LParen(Pos),
            Op(PinExprOpType),
        }

        #[derive(Debug)]
        enum StackItem {
            Value(Node),
            ConstExpr(Node),
            Operator(PinExprOpType),
        }

        let mut stack = Vec::new();
        let mut operator_stack = Vec::new();
        let mut expect_value = true;
        let mut closing_parens = 0;

        loop {
            let tk = self.peek_next_token()?;
            match tk.token_type() {
                tt::Ident(..) if expect_value => {
                    let node = NodeBuilder::identifier_from_token(self.consume_token()?);
                    stack.push(StackItem::Value(node));
                    expect_value = false;
                }
                tt::Punctuation(pt::LParen) if expect_value => {
                    self.consume_token()?;
                    operator_stack.push(OpItem::LParen(tk.pos()));
                }
                tt::Ident(..) | tt::Punctuation(pt::LParen) if !expect_value => {
                    self.add_error(self.expected_error_string("<operator>", &tk));
                    recover!();
                }
                tt::Punctuation(pt::RParen) if !expect_value && closing_parens > 0 => {
                    self.consume_token()?;
                    loop {
                        match operator_stack.last() {
                            Some(OpItem::LParen(_)) => {
                                stack.pop();
                                break;
                            }
                            Some(OpItem::Op(_)) => continue,
                            None => {
                                self.add_error(self.expr_error(
                                    ExprErrorKind::UnmatchedClosingParenthesis,
                                    tk.pos(),
                                ));
                                recover!();
                            }
                        }
                    }
                    closing_parens -= 1;
                }
                tt::Punctuation(pt::RParen) if expect_value && closing_parens > 0 => {
                    self.add_error(self.expected_error_string("<value>", &tk));
                    recover!();
                }
                tt::Punctuation(pt::Semicolon) => break,
                token => {
                    let Ok(op) = PinExprOpType::try_from(token) else {
                        break;
                    };

                    self.consume_token()?;

                    loop {
                        match operator_stack.last() {
                            Some(OpItem::Op(top_op)) => {
                                if top_op.higher_precedence(&op)
                                    || (op.associativity() == Associativity::Left
                                        && op.precedence() == top_op.precedence())
                                {
                                    let Some(OpItem::Op(top_op)) = operator_stack.pop() else {
                                        unreachable!()
                                    };

                                    stack.push(StackItem::Operator(top_op));
                                } else {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }

                    match op {
                        PinExprOpType::Index => {
                            let range = recover!(
                                self,
                                {
                                    let result = self.parse_constexpr().wrap()?;
                                    self.expect_next_token_is(punctuation!(RSquare))?;
                                    Ok(result)
                                },
                                [punctuation!(RSquare)]
                            )?;
                            stack.push(StackItem::ConstExpr(
                                range.unwrap_or_else(|| unreachable!()),
                            ));
                            self.expect_token_is(punctuation!(RSquare))?;
                        }
                        _ => (),
                    }

                    operator_stack.push(OpItem::Op(op));
                    expect_value = true;
                }
            }
        }

        for op in operator_stack.into_iter().rev() {
            match op {
                OpItem::Op(op) => stack.push(StackItem::Operator(op)),
                OpItem::LParen(pos) => {
                    // If we have an unmatched opening parenthesis, we just ignore it
                    // and continue parsing the rest of the expression.
                    // This is a best-effort approach to allow parsing to continue.
                    self.add_error(
                        self.expr_error(ExprErrorKind::UnmatchedOpeningParenthesis, pos),
                    );
                }
            }
        }

        fn _build_pinexpr_node_from_stack(
            parser: &mut Parser,
            stack: &mut Vec<StackItem>,
        ) -> AlwaysResult<Node> {
            match stack.pop() {
                Some(StackItem::Operator(op)) => {
                    let rhs = _build_pinexpr_node_from_stack(parser, stack)?;
                    let lhs = _build_pinexpr_node_from_stack(parser, stack)?;
                    let end_pos = rhs.pos();
                    Ok(NodeBuilder::new(&lhs)
                        .pin_expr(Some(lhs), op, rhs)
                        .build(&end_pos))
                }
                Some(StackItem::Value(node)) => Ok(node),
                Some(StackItem::ConstExpr(node)) => Ok(node),
                None => {
                    let tk = parser.peek_next_token()?;
                    parser.add_error(parser.expected_error_string("<expr>", &tk));
                    recover!()
                }
            }
        }

        if stack.is_empty() {
            let tk = self.peek_next_token()?;
            self.add_error(self.expected_error_string("<expr>", &tk));
            recover!();
        }

        let node = _build_pinexpr_node_from_stack(self, &mut stack)?;

        if stack.is_empty() {
            Ok(node)
        } else {
            unreachable!("should not be possible to get an invalid expression")
        }
    }

    fn parse_constexpr_with_starting_ident(&mut self, ident: Option<Token>) -> AlwaysResult<Node> {
        #[derive(Debug)]
        enum StackItem {
            Operator((ConstExprOpType, Pos)),
            Value(Node),
        }

        enum OpItem {
            LParen(Pos),
            Op((ConstExprOpType, Pos)),
        }
        let mut stack = Vec::new();
        let mut operator_stack = Vec::new();
        let mut expect_value = true;
        let mut closing_parens = 0;

        match ident {
            Some(ident) => stack.push(StackItem::Value(NodeBuilder::identifier_from_token(ident))),
            None => (),
        }

        loop {
            let tk = self.peek_next_token()?;
            match tk.token_type() {
                tt::Ident(..) if expect_value => {
                    let node = self.parse_path()?;
                    stack.push(StackItem::Value(node));
                    expect_value = false;
                }
                tt::Integer(..) if expect_value => {
                    let node = NodeBuilder::integer_from_token(self.consume_token()?);
                    stack.push(StackItem::Value(node));
                    expect_value = false;
                }
                tt::Punctuation(pt::LParen) if expect_value => {
                    self.consume_token()?;
                    operator_stack.push(OpItem::LParen(tk.pos()));
                    closing_parens += 1;
                }
                tt::Ident(..) | tt::Integer(..) | tt::Punctuation(pt::LParen) if !expect_value => {
                    self.add_error(self.expected_error_string("<operator>", &tk));
                    recover!();
                }
                tt::Punctuation(pt::RParen) if !expect_value && closing_parens > 0 => {
                    self.consume_token()?;
                    loop {
                        match operator_stack.last() {
                            Some(OpItem::LParen(_)) => {
                                stack.pop();
                                break;
                            }
                            Some(OpItem::Op(_)) => continue,
                            None => {
                                self.add_error(self.expr_error(
                                    ExprErrorKind::UnmatchedClosingParenthesis,
                                    tk.pos(),
                                ));
                                recover!();
                            }
                        }
                    }
                    closing_parens -= 1;
                }
                tt::Punctuation(pt::RParen) if expect_value && closing_parens > 0 => {
                    self.add_error(self.expected_error_string("<value>", &tk));
                    recover!();
                }
                token => {
                    let Ok(mut op) = ConstExprOpType::try_from(token) else {
                        break;
                    };

                    if op == ConstExprOpType::Range {
                    } else if expect_value {
                        op = match op {
                            op if op.unary_equivalent().is_some() => op.unary_equivalent().unwrap(),
                            op if op.associativity() == Associativity::Unary => op,
                            _ => {
                                self.add_error(self.expected_error_string("<value>", &tk));
                                recover!();
                            }
                        };
                    } else if op.associativity() == Associativity::Unary {
                        self.add_error(self.expr_error(
                            ExprErrorKind::ExpectedBinaryOperator(format!("{}", tk.value())),
                            tk.pos(),
                        ));
                        recover!();
                    }

                    let op_pos = self.consume_token()?.pos();

                    loop {
                        match operator_stack.last() {
                            Some(OpItem::Op((top_op, _))) => {
                                if top_op.higher_precedence(&op)
                                    || (op.associativity() == Associativity::Left
                                        && op.precedence() == top_op.precedence())
                                {
                                    let Some(OpItem::Op((top_op, pos))) = operator_stack.pop()
                                    else {
                                        unreachable!()
                                    };

                                    stack.push(StackItem::Operator((top_op, pos)));
                                } else {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }

                    operator_stack.push(OpItem::Op((op, op_pos)));
                    expect_value = true;
                }
            }
        }

        for op in operator_stack.into_iter().rev() {
            match op {
                OpItem::Op(op) => stack.push(StackItem::Operator(op)),
                OpItem::LParen(pos) => {
                    // If we have an unmatched opening parenthesis, we just ignore it
                    // and continue parsing the rest of the expression.
                    // This is a best-effort approach to allow parsing to continue.
                    self.add_error(
                        self.expr_error(ExprErrorKind::UnmatchedOpeningParenthesis, pos),
                    );
                }
            }
        }

        fn _build_constexpr_node_from_stack(
            parser: &mut Parser,
            stack: &mut Vec<StackItem>,
        ) -> AlwaysResult<Node> {
            match stack.pop() {
                Some(StackItem::Operator((ConstExprOpType::Range, pos))) => {
                    // I don't know why we parse lhs first but it works and parsing rhs first
                    // doesn't
                    let lhs = if stack.is_empty() {
                        None
                    } else {
                        Some(_build_constexpr_node_from_stack(parser, stack)?)
                    };

                    let rhs = if stack.is_empty() {
                        None
                    } else {
                        Some(_build_constexpr_node_from_stack(parser, stack)?)
                    };

                    let start_pos = lhs.as_ref().map_or(pos, |lhs| lhs.pos());
                    let end_pos = rhs.as_ref().map_or(pos, |rhs| rhs.pos());

                    Ok(NodeBuilder::new(&start_pos)
                        .const_expr_range(lhs, rhs)
                        .build(&end_pos))
                }
                Some(StackItem::Operator((op, _)))
                    if op.associativity() == Associativity::Unary =>
                {
                    let rhs = _build_constexpr_node_from_stack(parser, stack)?;
                    let end_pos = rhs.pos();
                    Ok(NodeBuilder::new(&rhs)
                        .const_expr_unary(op, rhs)
                        .build(&end_pos))
                }
                Some(StackItem::Operator((op, _))) => {
                    let rhs = _build_constexpr_node_from_stack(parser, stack)?;
                    let lhs = _build_constexpr_node_from_stack(parser, stack)?;
                    let end_pos = rhs.pos();
                    Ok(NodeBuilder::new(&lhs)
                        .const_expr_binary(lhs, op, rhs)
                        .build(&end_pos))
                }
                Some(StackItem::Value(node)) => Ok(node),
                None => {
                    let tk = parser.peek_next_token()?;
                    parser.add_error(parser.expected_error_string("<constexpr>", &tk));
                    recover!()
                }
            }
        }

        if stack.is_empty() {
            let tk = self.peek_next_token()?;
            self.add_error(self.expected_error_string("<constexpr>", &tk));
            recover!();
        }

        let node = _build_constexpr_node_from_stack(self, &mut stack)?;
        if stack.is_empty() {
            Ok(node)
        } else {
            unreachable!("should not be possible to get an invalid expression")
        }
    }

    fn parse_constexpr(&mut self) -> AlwaysResult<Node> {
        self.parse_constexpr_with_starting_ident(None)
    }

    fn parse_constexpr_maybe_empty(&mut self) -> ParseResult<Node> {
        match self.peek_next_token()?.token_type() {
            tt::Integer(..) | tt::Ident(..) | tt::Punctuation(pt::LParen) => {
                self.parse_constexpr().wrap()
            }
            tk if ConstExprOpType::try_from(tk)
                .is_ok_and(|op| op.is_unary() || op.unary_equivalent().is_some()) =>
            {
                self.parse_constexpr().wrap()
            }
            _ => ParseResult::none(),
        }
    }

    fn has_next_token(&mut self) -> bool {
        !self.tokens.peek().is_none()
    }

    fn peek_next_token(&mut self) -> AlwaysResult<Token> {
        match self.tokens.peek() {
            Some(token) => Ok((*token).clone()),
            None => Err(ParseFailKind::Critical(diagnostic!(
                Error,
                ParserError::UnexpectedEof,
                pos = Pos::Module(self.module)
            ))),
        }
    }

    fn consume_until(&mut self, tts: &[TokenType]) -> AlwaysResult<Token> {
        let tk = loop {
            let tk = match self.peek_next_token().catch()? {
                Caught::Ok(tk) => tk,
                Caught::Recover => continue,
            };

            let next_token_type = tk.token_type().clone();

            if tts.iter().any(|tt| tt == &next_token_type) {
                break tk;
            }

            self.consume_token()?;
        };

        Ok(tk)
    }

    fn consume_until_allow_eof(&mut self, tts: &[TokenType]) -> Option<Token> {
        let tk = loop {
            let tk = match self.peek_next_token() {
                Ok(tk) => tk,
                Err(_) => break None,
            };

            let next_token_type = tk.token_type().clone();

            if tts.iter().any(|tt| tt == &next_token_type) {
                break Some(tk);
            }

            self.consume_token()
                .unwrap_or_else(|_| unreachable!("eof should be allowed"));
        };

        tk
    }

    fn consume_token(&mut self) -> AlwaysResult<Token> {
        match self.tokens.next() {
            Some(tk) => {
                self.last_token_pos = tk.pos();
                Ok(tk.clone())
            }
            None => Err(ParseFailKind::Critical(diagnostic!(
                Error,
                ParserError::UnexpectedEof,
                pos = Pos::Module(self.module)
            ))),
        }
    }

    fn assert_token_is(&mut self, tt: TokenType) -> Token {
        let tk = self.consume_token();
        match tk {
            Ok(tk) => {
                assert!(tk.token_type() == &tt);
                tk.clone()
            }
            Err(e) => panic!("unexpected end of file: {e:#?}"),
        }
    }

    fn expr_error(&self, kind: ExprErrorKind, pos: Pos) -> Diagnostic {
        diagnostic!(Error, ParserError::ExprError(kind), pos = pos,)
    }

    fn expected_error_string(&self, expected: &str, got: &Token) -> Diagnostic {
        diagnostic!(
            Error,
            ParserError::Expected(expected.to_string(), got.value().to_string(),),
            pos = got.pos(),
        )
    }

    fn expected_error(&self, expected: &[TokenType], got: &Token) -> Diagnostic {
        let mut expected = expected
            .iter()
            .map(|tt| match tt {
                tt::Ident(..) => "identifier".to_string(),
                tt::Integer(..) => "integer".to_string(),
                tt::Keyword(kw) => format!("'{}'", kw.value()),
                tt::Punctuation(punc) => format!("'{}'", punc.value()),
            })
            .collect::<Vec<_>>();

        let last = expected
            .pop()
            .expect("argument 'expected' passed to 'expected_error' was empty!");
        let expected = if expected.is_empty() {
            last
        } else {
            let mut joined_tts = expected.join(", ");
            joined_tts.push_str(&format!(" or {last}"));
            joined_tts
        };

        self.expected_error_string(&expected, got)
    }

    fn expect_next_token_is(&mut self, tt: TokenType) -> AlwaysResult<Token> {
        self.expect_next_token_one_of(&[tt])
    }

    fn expect_token_is(&mut self, tt: TokenType) -> AlwaysResult<Token> {
        self.expect_token_one_of(&[tt])
    }

    // fn expect_token_is_else_consume(
    //     &mut self,
    //     tt: TokenType,
    // ) -> Result<ParseResult<Token>, CompilerError> {
    //     self.expect_token_one_of_else_consume(&[tt])
    // }

    // fn expect_token_is_else_consume_if(
    //     &mut self,
    //     tt: TokenType,
    //     consume_if: &[TokenType],
    // ) -> Result<ParseResult<Token>, CompilerError> {
    //     self.expect_token_one_of_else_consume_if(&[tt], consume_if)
    // }

    // fn expect_token_is_else_consume_if_not(
    //     &mut self,
    //     tt: TokenType,
    //     consume_if: &[TokenType],
    // ) -> Result<ParseResult<Token>, CompilerError> {
    //     self.expect_token_one_of_else_consume_if_not(&[tt], consume_if)
    // }

    fn expect_next_token_one_of(&mut self, tts: &[TokenType]) -> AlwaysResult<Token> {
        let tk = self.peek_next_token()?;
        let token_type = tk.token_type();

        if tts.iter().any(|tt| token_type == tt) {
            return Ok(tk);
        }

        self.add_error(self.expected_error(tts, &tk));

        recover!()
    }

    fn expect_token_one_of(&mut self, tts: &[TokenType]) -> AlwaysResult<Token> {
        let tk = self.peek_next_token()?;
        let token_type = tk.token_type();

        if tts.iter().any(|tt| token_type == tt) {
            return self.consume_token();
        }

        self.add_error(self.expected_error(tts, &tk));

        recover!()
    }

    fn add_error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn consume_optional_semicolon(&mut self) -> Option<Token> {
        if !self.has_next_token() {
            return None;
        };

        match self.peek_next_token() {
            Ok(tk) if tk.token_type() == &punctuation!(Semicolon) => {
                Some(self.consume_token().unwrap())
            }
            _ => None,
        }
    }
}

pub struct NodeBuilder {
    node_type: Option<NodeType>,
    pos: Pos,
}

impl NodeBuilder {
    pub fn new(start_position: &dyn Position) -> Self {
        Self {
            node_type: None,
            pos: start_position.pos(),
        }
    }

    pub fn from_token(node_type: NodeType, token: &Token) -> Node {
        Node::new(node_type, token.pos())
    }

    pub fn build(self, end_position: &dyn Position) -> Node {
        Node::new(self.node_type.unwrap(), self.pos.expand(end_position.pos()))
    }

    pub fn node_type(mut self, node_type: NodeType) -> Self {
        self.node_type = Some(node_type);
        self
    }

    pub fn identifier_from_token(token: Token) -> Node {
        if let tt::Ident(name) = token.token_type() {
            Self::from_token(NodeType::Identifier(*name), &token)
        } else {
            unreachable!(
                "expected token to be an identifier, but got {:?}",
                token.token_type()
            )
        }
    }

    pub fn path_from_tokens(tokens: Vec<Token>) -> Node {
        if tokens.is_empty() {
            unreachable!("path_from_tokens given empty list");
        }

        let nodes: Vec<Node> = tokens
            .into_iter()
            .map(Self::identifier_from_token)
            .collect();

        let start = nodes.first().unwrap().pos();
        let end = nodes.last().unwrap().pos();

        Self::new(&start)
            .node_type(NodeType::Path(nodes))
            .build(&end)
    }

    pub fn integer_from_token(token: Token) -> Node {
        if let tt::Integer(value) = token.token_type() {
            Self::from_token(NodeType::Integer(*value), &token)
        } else {
            unreachable!(
                "expected token to be an integer, but got {:?}",
                token.token_type()
            )
        }
    }

    pub fn const_decl(self, r#type: Node, name: Node, expr: Node) -> Self {
        self.node_type(NodeType::ConstDecl {
            r#type: Box::new(r#type),
            name: Box::new(name),
            expr: Box::new(expr),
        })
    }

    pub fn circ(self, name: Node, args: Vec<Node>, children: Vec<Node>) -> Self {
        self.node_type(NodeType::Circ {
            name: Box::new(name),
            args,
            children,
        })
    }

    pub fn circ_arg(self, r#type: Node, name: Node, default: Option<Node>) -> Self {
        self.node_type(NodeType::CircArg {
            r#type: Box::new(r#type),
            name: Box::new(name),
            default: default.map(Box::new),
        })
    }

    pub fn r#enum(self, name: Node, variants: Vec<Node>) -> Self {
        self.node_type(NodeType::Enum {
            name: Box::new(name),
            variants,
        })
    }

    pub fn statement(self, statement: Node) -> Self {
        self.node_type(NodeType::Statement(Box::new(statement)))
    }

    pub fn r#type(self, name: Node, args: Vec<Node>) -> Self {
        self.node_type(NodeType::Type {
            name: Box::new(name),
            args,
        })
    }

    pub fn type_arg(self, name: Option<Node>, value: Node) -> Self {
        self.node_type(NodeType::TypeArg {
            name: name.map(Box::new),
            value: Box::new(value),
        })
    }

    pub fn pin_expr(self, lhs: Option<Node>, op: PinExprOpType, rhs: Node) -> Self {
        self.node_type(NodeType::PinExpr {
            lhs: lhs.map(Box::new),
            op,
            rhs: Box::new(rhs),
        })
    }

    pub fn connection(self, lhs: Node, direction: ConnectionDirection, rhs: Node) -> Self {
        self.node_type(NodeType::Connection {
            lhs: Box::new(lhs),
            direction,
            rhs: Box::new(rhs),
        })
    }

    pub fn const_expr(self, lhs: Option<Node>, op: ConstExprOpType, rhs: Option<Node>) -> Self {
        self.node_type(NodeType::ConstExpr {
            lhs: lhs.map(Box::new),
            op,
            rhs: rhs.map(Box::new),
        })
    }

    pub fn const_expr_unary(self, op: ConstExprOpType, rhs: Node) -> Self {
        self.const_expr(None, op, Some(rhs))
    }

    pub fn const_expr_binary(self, lhs: Node, op: ConstExprOpType, rhs: Node) -> Self {
        self.const_expr(Some(lhs), op, Some(rhs))
    }

    pub fn const_expr_range(self, lhs: Option<Node>, rhs: Option<Node>) -> Self {
        self.const_expr(lhs, ConstExprOpType::Range, rhs)
    }

    pub fn assert(self, constexpr: Node) -> Self {
        self.node_type(NodeType::Assert(Box::new(constexpr)))
    }

    pub fn import(self, name: Node) -> Self {
        self.node_type(NodeType::Import(Box::new(name)))
    }

    pub fn r#if(self, condition: Node, then: Vec<Node>, r#else: Vec<Node>) -> Self {
        self.node_type(NodeType::If {
            condition: Box::new(condition),
            then,
            r#else,
        })
    }

    pub fn decl_from_name(name: Node) -> Node {
        let pos = name.pos();
        Self::new(&pos)
            .node_type(NodeType::Decl {
                name: Box::new(name),
                count: None,
            })
            .build(&pos)
    }

    pub fn decl(self, name: Node, count: Node) -> Self {
        self.node_type(NodeType::Decl {
            name: Box::new(name),
            count: Some(Box::new(count)),
        })
    }

    pub fn decls(self, r#type: Node, decls: Vec<Node>) -> Self {
        self.node_type(NodeType::Decls {
            r#type: Box::new(r#type),
            decls,
        })
    }

    pub fn pin_decls(self, direction: PinDirection, decls: Vec<Node>) -> Self {
        self.node_type(NodeType::PinDecls { direction, decls })
    }

    pub fn with(self, decls: Vec<Node>) -> Self {
        self.node_type(NodeType::With(decls))
    }

    pub fn r#use(self, path: Node, name: Option<Node>) -> Self {
        self.node_type(NodeType::Use {
            path: Box::new(path),
            as_name: name.map(Box::new),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::tokeniser::Tokeniser;
    use crate::{util::Interner, Loader};

    #[test]
    fn parse_constexpr() {
        let module = Loader::ROOT_MODULE;
        let mut interner = Interner::new();
        let tokens = Tokeniser::new(module, "-!xyz:Enum:N + 5", &mut interner)
            .tokenise()
            .unwrap();
        let constexpr_node = Parser::new(module, tokens.tokens()).parse_constexpr();
        println!("{constexpr_node:#?}");
    }
}
