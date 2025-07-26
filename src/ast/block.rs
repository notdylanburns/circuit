use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, PunctuationType as pt};
use super::{NodeRef, NodeTrait};
use super::statement::parse_statement;

#[derive(Debug, Default)]
pub struct BlockNode {
    pub stmts: Vec<NodeRef>,
}

impl NodeTrait for BlockNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start;
        let end;

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::LCurly), loc, .. } => start = loc,
            _ => unreachable!(),
        };

        loop {
            match tokens.peek()? {
                Token { token_type: tt::Punctuation(pt::RCurly), .. } => break,
                _ => node.stmts.push(parse_statement(tokens)?),
            }
        };

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::RCurly), loc, span } => end = loc + span,
            _ => unreachable!(),
        };

        Ok((start, end, node))
    }
}