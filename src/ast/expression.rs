use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, PunctuationType as pt};
use super::{Node, NodeRef, NodeTrait};
use super::{constexpr::ConstExprNode, ident::IdentNode};

/*
Expressions:
abc[1]
bcd.a12
asd.cde[0][1].ef[2:5]
*/

#[derive(Debug)]
pub enum ExpressionPart {
    Child(NodeRef),
    Index(NodeRef),
    Range(NodeRef, NodeRef),
}

impl NodeTrait for ExpressionPart {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::Period), loc, .. } => {
                let node = Node::parse::<IdentNode>(tokens)?;
                Ok((loc, node.end, ExpressionPart::Child(node.as_ref())))
            },
            Token { token_type: tt::Punctuation(pt::LSquare), loc, .. } => {
                let start = loc;
                let first = Node::parse::<ConstExprNode>(tokens)?.as_ref();   // constexpr
                let node = match tokens.peek()? {
                    Token { token_type: tt::Punctuation(pt::Colon), .. } => {
                        tokens.next()?;
                        let second = Node::parse::<ConstExprNode>(tokens)?.as_ref();   // constexpr
                        Self::Range(first, second)
                    }
                    _ => Self::Index(first),
                };
                match tokens.next()? {
                    Token { token_type: tt::Punctuation(pt::RSquare), loc, span } => Ok((start, loc + span, node)),
                    tk => Err(CktError::UnexpectedToken((line!(), file!()), tk)),
                }
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ExpressionNode {
    pub base: NodeRef,
    pub parts: Vec<NodeRef>,
}

impl NodeTrait for ExpressionNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let start;
        let mut end;

        match tokens.peek()? {
            Token { loc, span, .. } => {
                start = *loc;
                end = *loc + *span;
            },
        };

        match tokens.peek()? {
            Token { token_type: tt::Ident(_), .. } => node.base = Node::parse::<IdentNode>(tokens)?.as_ref(),
            _ => ()
        };
        loop {
            match tokens.peek()? {
                Token { token_type: tt::Punctuation(pt::Period | pt::LSquare), .. } => {
                    let new_part = Node::parse::<ExpressionPart>(tokens)?;
                    end = new_part.end;
                    node.parts.push(new_part.as_ref());
                },
                _ => break,
            };
        };

        Ok((start, end, node))
    }
}