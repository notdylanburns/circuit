use crate::error::CktError;
use crate::token::{TokenStream, Token, TokenType as tt, PunctuationType as pt};
use super::{NodeRef, Node};
use super::ident::IdentNode;


pub fn parse_params(tokens: &mut TokenStream) -> Result<(usize, usize, Vec<NodeRef>), CktError> {
    let start;
    match tokens.next()? {
        Token { token_type: tt::Punctuation(pt::LAngle), loc, .. } => start = loc,
        _ => unreachable!(), // We check this when parsing the parent
    };

    let mut nodes = Vec::new();

    let end = loop {
        nodes.push(Node::parse::<IdentNode>(tokens)?.as_ref());
        match tokens.next()? {
            Token { token_type: tt::Punctuation(pt::Comma), .. } => continue,
            Token { token_type: tt::Punctuation(pt::RAngle), loc, span } => break loc + span,
            tk => return Err(CktError::UnexpectedToken((line!(), file!()), tk)),
        };
    };

    Ok((start, end, nodes))
}