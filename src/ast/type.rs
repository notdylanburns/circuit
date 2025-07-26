use crate::error::CktError;
use crate::token::{TokenStream, Token, TokenType as tt, PunctuationType as pt};
use super::{NodeRef, Node, NodeTrait};
use super::{args::parse_args, expression::ExpressionNode, constexpr::ConstExprNode};

#[derive(Debug, Default)]
pub struct TypeNode {
    pub r#type: NodeRef,
    pub vars: Vec<NodeRef>,
    pub width: NodeRef,
}

impl NodeTrait for TypeNode {
    fn parse(tokens: &mut TokenStream) -> Result<(usize, usize, Self), CktError> {
        let mut node = Self::default();
        let mut end;

        let mut array_type = false;

        let start = match tokens.peek()? {
            Token { token_type: tt::Punctuation(pt::LSquare), .. } => {
                array_type = true;
                match tokens.next()? {
                    Token { token_type: tt::Punctuation(pt::LSquare), loc, .. } => loc,
                    _ => unreachable!(),
                }
            },
            Token { loc, .. } => *loc,
        };

        node.r#type = if array_type {
            Node::parse::<TypeNode>(tokens)?.as_ref()
        } else {
            Node::parse::<ExpressionNode>(tokens)?.as_ref()
        };

        end = node.r#type.as_ref().unwrap().end;
        match tokens.peek()? {
            Token { token_type: tt::Punctuation(pt::LAngle), ..} => (_, end, node.vars) = parse_args(tokens)?,
            _ => (),
        }

        if !array_type {
            return Ok((start, end, node));
        }

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::SemiColon), .. } => (),
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk)),
        };

        node.width = Node::parse::<ConstExprNode>(tokens)?.as_ref();

        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::RSquare), loc, span } => Ok((start, loc + span, node)),
            tk => Err(CktError::UnexpectedToken((line!(), file!()), tk)),
        }
    }
}