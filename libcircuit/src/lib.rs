#[macro_use]
mod ansi;
mod diagnostics;
mod util;

pub use diagnostics::{Diagnostic, Diagnostics};
pub use util::LinkedStack;

mod loader;
pub use loader::{Loader, Module, ModuleId};

mod tokeniser;
pub use tokeniser::{IdentId, Token, Tokeniser};

#[cfg(feature = "parser")]
mod ast;
#[cfg(feature = "parser")]
mod expr;
#[cfg(feature = "parser")]
mod parser;
#[cfg(feature = "parser")]
pub use {ast::AST, parser::Parser};

#[cfg(feature = "analyser")]
mod analyser;
#[cfg(feature = "analyser")]
pub use analyser::Analyser;
