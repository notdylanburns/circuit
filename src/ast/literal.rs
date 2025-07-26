use crate::error::CktError;
use crate::token::{TokenStream, Token, TokenType as tt};
use super::NodeTrait;


#[derive(Debug)]
pub enum LiteralNode {
    Numeric(usize),
}

impl NodeTrait for LiteralNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        match tokens.next()? {
            Token { token_type: tt::Number(v), loc, span } => Ok((loc, loc + span, Self::Numeric(v))),
            tk => Err(CktError::UnexpectedToken((line!(), file!()), tk))
        }
    }
}