#![allow(unused)]

mod error;
mod ref_util;
#[macro_use]
mod file;
mod token;
mod lexer;
mod ast;
mod objects;
mod sim;

use std::io::Write;
use error::CktError;
use file::File;

fn wrapper() -> Result<(), CktError> {
    let file = File::new("example.ckt".to_string())?.as_ref();
    let tokens = lexer::lex_file(file)?;
    // println!("{tokens:#?}");

    let ast = ast::AST::new(tokens)?;
    println!("{ast}");

    Ok(())
}

fn main() {
    match wrapper() {
        Ok(_) => (),
        Err(e) => { writeln!(std::io::stderr(), "[ERROR] {}", e); },
    }    
}
