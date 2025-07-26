use crate::error::CktError;
use crate::token::{TokenStream, TokenType as tt, KeywordType as kt};
use super::{Node, NodeRef, NodeTrait};
use super::{circuit::CircuitNode, statement::UseNode};

#[derive(Debug, Default)]
pub struct ProgramNode {
    pub children: Vec<NodeRef>,
}

impl NodeTrait for ProgramNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();

        let start = match tokens.peek() {
            Ok(tk) => tk.loc,
            Err(_) => 0,                            // No tokens
        };

        loop {
            let tk = match tokens.peek() {
                Ok(tk) => tk,
                Err(_) => break,  // No more tokens
            };

            match &tk.token_type {
                tt::Keyword(kt::Circ) => node.children.push(
                    Node::parse::<CircuitNode>(tokens)?.as_ref()
                ),
                tt::Keyword(kt::Use) => node.children.push(
                    Node::parse::<UseNode>(tokens)?.as_ref()
                ),
                _ => break //TODO: uncomment return Err(CktError::UnexpectedToken())
            }
        }

        let end = node.children.last().map_or_else(|| 0, |t| t.as_ref().unwrap().end);
        Ok((start, end, node))
    }
}