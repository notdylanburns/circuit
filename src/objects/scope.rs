use std::rc::Rc;

use super::symbol::Symbol;

pub struct Scope {
    parent: Option<Rc<Scope>>,
    symbols: Vec<Symbol>,
}