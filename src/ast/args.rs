use crate::error::CktError;
use crate::token::{TokenStream, Token, TokenType as tt, PunctuationType as pt};
use super::{NodeRef, Node};
use super::constexpr::ConstExprNode;


pub fn parse_args(tokens: &mut TokenStream) -> Result<(usize, usize, Vec<NodeRef>), CktError> {
    let start;
    match tokens.next()? {
        Token { token_type: tt::Punctuation(pt::LAngle), loc, .. } => start = loc,
        _ => unreachable!(), // We check this when parsing the parent
    };

    let mut nodes = Vec::new();

    let end = loop {
        nodes.push(Node::parse::<ConstExprNode>(tokens)?.as_ref());
        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::Comma), .. } => continue,
            Token { token_type: tt::Punctuation(pt::RAngle), loc, span } => break loc + span,
            tk => return Err(CktError::ExpectedToken(vec![
                tt::Punctuation(pt::Comma),
                tt::Punctuation(pt::RAngle)
            ]).received(tk.token_type)),
        };
    };

    Ok((start, end, nodes))
}