use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, PunctuationType as pt, KeywordType as kt};
use super::{Node, NodeRef, NodeTrait};
use super::super::ident::IdentNode;

#[derive(Debug)]
pub enum PathElem {
    Module(String),
    Dot(usize),
}

#[derive(Debug, Default)]
pub struct UseNode {
    pub path: Vec<PathElem>,
    pub name: NodeRef,
}

impl NodeTrait for UseNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start;

        match tokens.next()? {
            Token { token_type: tt::Keyword(kt::Use), loc, .. } => start = loc,
            _ => unreachable!(),
        };

        loop {
            match tokens.peek()? {
                Token { token_type: tt::Punctuation(pt::Period), .. } => {
                    tokens.next()?;
                    let previous = node.path.last_mut();
                    match previous {
                        Some(PathElem::Dot(i)) => *previous.unwrap() = PathElem::Dot(*i + 1),
                        _ => node.path.push(PathElem::Dot(0)),
                    };
                },
                Token { token_type: tt::Ident(_), .. } => {
                    match tokens.next()? {
                        Token { token_type: tt::Ident(name), .. } => node.path.push(PathElem::Module(name)),
                        _ => unreachable!(),
                    };
                },
                Token { token_type: tt::Punctuation(pt::SemiColon), .. } => break,
                Token { token_type: tt::Keyword(kt::As), .. } => {
                    tokens.next()?;
                    node.name = Node::parse::<IdentNode>(tokens)?.as_ref();
                    break;
                },
                _ => return Err(CktError::UnexpectedToken((line!(), file!()), tokens.next()?)),
            };
        };

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::SemiColon), loc, span } => Ok((start, loc + span, node)),
            _ => return Err(CktError::UnexpectedToken((line!(), file!()), tokens.next()?)),
        }
    }
}