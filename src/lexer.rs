use super::error::CktError;
use super::file::{FileRef, file_ref};
use super::token::{Token, TokenStream};

fn add_token(tokens: &mut Vec<Token>, string: &Vec<u8>, loc: usize, file: &FileRef) -> Result<(), CktError> {
    let token_len = string.len();
    if token_len != 0 {
        match Token::new(string, loc, token_len) {
            Some(tok) => tokens.push(tok),
            None => return Err(CktError::InvalidToken(string.to_vec()).at_location(file.clone(), loc, token_len))
        }
    };

    Ok(())
}

pub fn lex_file(file: FileRef) -> Result<TokenStream, CktError> {
    let input = {   // This is in a block to drop the borrowed RefMut after this statement
        file.borrow_mut().read_all()?
    };
    let mut tokens = Vec::with_capacity(input.len() / 4);    // Divide by rough average token length

    let mut string = Vec::with_capacity(255);
    let mut start_loc: usize = 0;
    for (loc, byte) in input.iter().peekable().enumerate() {
        if string.len() == 0 {
            start_loc = loc
        }

        let c = *byte;
        if Token::is_whitespace(c) {
            add_token(&mut tokens, &string, start_loc, &file)?;
            string.truncate(0);

            continue;
        };

        if Token::is_token_break(c) {
            add_token(&mut tokens, &string, start_loc, &file)?;
            string.truncate(0);

            add_token(&mut tokens, &vec![c], loc, &file)?;
            continue;
        };

        string.push(c);
    }
    add_token(&mut tokens, &string, start_loc, &file)?;

    Ok(TokenStream {
        tokens: tokens.into_iter().peekable(),
        source: file,
    })
}