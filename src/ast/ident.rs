use crate::error::CktError;
use crate::token::{TokenStream, Token, TokenType as tt};
use super::NodeTrait;

#[derive(Debug, Default)]
pub struct IdentNode {
    pub name: String,
}

impl NodeTrait for IdentNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start;
        let end;

        match tokens.next()? {
            Token { token_type: tt::Ident(s), loc, span } => {
                node.name = s;
                start = loc;
                end = loc + span;
            },
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk))
        }

        Ok((start, end, node))
    }
}