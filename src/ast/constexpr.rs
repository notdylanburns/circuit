use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt};
use super::literal::LiteralNode;
use super::{Node, NodeRef, NodeTrait};
use super::ident::IdentNode;

// #[derive(Debug)]
// enum Operator {
//     Write,
// }

#[derive(Debug)]
pub struct ConstExprNode {
    pub lhs: NodeRef,
    // pub op: Operator,
    // pub rhs: NodeRef,
}

impl NodeTrait for ConstExprNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let node = match tokens.peek()? {
            Token { token_type: tt::Ident(_), .. } => Node::parse::<IdentNode>(tokens)?,
            Token { token_type: tt::Number(_), .. } => Node::parse::<LiteralNode>(tokens)?,
            _ => return Err(CktError::UnexpectedToken((line!(), file!()), tokens.next()?)),
        };

        Ok((node.start, node.end, Self { lhs: node.as_ref() }))
    }
}