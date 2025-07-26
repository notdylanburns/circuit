use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, PunctuationType as pt, KeywordType as kt};
use super::{Node, NodeRef, NodeTrait};
use super::super::{ident::IdentNode, r#type::TypeNode};

#[derive(Debug, Default)]
pub struct DeclNode {
    pub name: NodeRef,
    pub r#type: NodeRef,
}

impl NodeTrait for DeclNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start;

        match tokens.next()? {
            Token { token_type: tt::Keyword(kt::Let), loc, .. } => start = loc,
            _ => unreachable!(),
        };

        node.name = Node::parse::<IdentNode>(tokens)?.as_ref();
        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::Colon), .. } => node.r#type = Node::parse::<TypeNode>(tokens)?.as_ref(),
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk))
        }

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::SemiColon), loc, span } => Ok((start, loc + span, node)),
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk))
        }
    }
}