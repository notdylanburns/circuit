use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, PunctuationType as pt};
use super::{Node, NodeRef, NodeTrait};
use super::super::expression::ExpressionNode;

#[derive(Debug)]
pub enum Operator {
    Write,
}

#[derive(Debug)]
pub struct OperationNode {
    pub lhs: NodeRef,
    pub op: Operator,
    pub rhs: NodeRef,
}

impl NodeTrait for OperationNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let lhs = Node::parse::<ExpressionNode>(tokens)?;
        let start = lhs.start;

        let op = match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::RAngle), .. } => Operator::Write,
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk)),
        };

        let rhs = Node::parse::<ExpressionNode>(tokens)?;
        let node = Self {
            lhs: lhs.as_ref(),
            op,
            rhs: rhs.as_ref()
        };

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::SemiColon), loc, span } => Ok((start, loc + span, node)),
            tk => Err(CktError::UnexpectedToken((line!(), file!()), tk)),
        }
    }
}