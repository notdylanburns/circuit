mod decl;
mod operation;
mod r#use;

use crate::error::CktError;
use crate::token::{Token, TokenStream, TokenType as tt, KeywordType as kt};
use super::{Node, NodeRef, NodeTrait};

pub use decl::DeclNode;
pub use operation::{OperationNode, Operator};
pub use r#use::UseNode;

/*
    Declarations: IDENT COLON TYPE
    Operation: EXPR OP EXPR
    Rep: REP LITERAL|IDENT BLOCK
    Use: 
*/

pub fn parse_statement(tokens: &mut TokenStream) -> Result<NodeRef, CktError> {
    match tokens.peek()? {
        Token { token_type: tt::Keyword(kt::Let), .. } => Ok(Node::parse::<decl::DeclNode>(tokens)?.as_ref()),
        Token { token_type: tt::Keyword(kt::Use), .. } => Ok(Node::parse::<r#use::UseNode>(tokens)?.as_ref()),
        _ => Ok(Node::parse::<operation::OperationNode>(tokens)?.as_ref()),
    }
}