use crate::error::CktError;
use crate::token::{TokenStream, TokenType as tt, KeywordType as kt, PunctuationType as pt, Token};
use super::{Node, NodeRef, NodeTrait};
use super::{block::BlockNode, ident::IdentNode, params::parse_params};

#[derive(Debug, Default)]
pub struct CircuitNode {
    pub name: NodeRef,
    pub params: Vec<NodeRef>,
    pub block: NodeRef,
}

impl NodeTrait for CircuitNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start = match tokens.next()? {
            Token { token_type: tt::Keyword(kt::Circ), loc, ..} => loc,
            _ => unreachable!() // We check this when parsing the program
        };

        if tokens.peek()?.token_type != tt::Punctuation(pt::LCurly) {
            // Name is optional, for the main circuit no name is expected
            node.name = Node::parse::<IdentNode>(tokens)?.as_ref();

            match tokens.peek()? {
                Token { token_type: tt::Punctuation(pt::LAngle), .. } => (_, _, node.params) = parse_params(tokens)?,
                _ => ()
            };
        }

        let next_token = tokens.peek()?;
        match &next_token.token_type {
            tt::Punctuation(pt::LCurly) => node.block = Node::parse::<BlockNode>(tokens)?.as_ref(),
            tk => return Err(CktError::ExpectedToken(vec![tt::Punctuation(pt::LCurly)]).received(tk.clone()))
        }

        let end = node.block.as_ref().unwrap().end;

        Ok((start, end, node))
    }
}