use super::constexpr::{ConstType, ConstValue};
use super::local::LocalType;
use crate::ast::Node;
use crate::loader::ModuleId;
use crate::tokeniser::IdentId;
use crate::util::{Interner, OrderedMap, Pos, Position};

use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct CircArg {
    pub name: IdentId,
    pub r#type: ConstType,
    pub default: Option<ConstValue>,
    pub pos: Pos,
}

#[derive(Debug)]
pub(super) enum CircKind {
    Module(Vec<Node>),
    Library {
        external_circ_id: usize,
        initialise: circuit_extlib::InitialiserFn,
        get_meta: circuit_extlib::GetMetaFn,
    },
}

#[derive(Debug)]
pub(super) struct CircSymbol {
    pub module_id: ModuleId,
    pub name: IdentId,
    pub args: OrderedMap<IdentId, CircArg>,
    pub kind: CircKind,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ConstSymbol {
    pub name: IdentId,
    pub r#type: ConstType,
    pub value: ConstValue,
    pub pos: Pos,
}

#[derive(Debug)]
pub(super) struct EnumSymbol {
    pub module_id: ModuleId,
    pub name: IdentId,
    pub variants: HashMap<IdentId, Pos>,
    pub pos: Pos,
}

impl EnumSymbol {
    const fn get_type(&self) -> ConstType {
        ConstType::Enum {
            module_id: self.module_id,
            enum_name: self.name,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct LocalSymbol {
    pub name: IdentId,
    pub r#type: LocalType,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ModuleSymbol {
    pub name: IdentId,
    pub module_id: ModuleId,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ScopeSymbolType {
    Circ,
    Const,
    EnumType,
    PrimitiveType,
    Local,
    Module,
}

impl ScopeSymbolType {
    pub fn error_msg_str(&self) -> &'static str {
        match self {
            Self::Circ => "circuit",
            Self::Const => "constant",
            Self::EnumType => "enum",
            Self::PrimitiveType => "primitive type",
            Self::Local => "local",
            Self::Module => "module",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum ScopeSymbol {
    Circ(Rc<CircSymbol>),
    Const(ConstSymbol),
    EnumType(Rc<EnumSymbol>),
    EnumValue {
        module_id: ModuleId,
        enum_name: IdentId,
        variant: IdentId,
    },
    PrimitiveType(ConstType),
    Local(LocalSymbol),
    Module(ModuleSymbol),
}

impl ScopeSymbol {
    pub fn name(&self) -> IdentId {
        match self {
            Self::Circ(circ) => circ.name,
            Self::Const(r#const) => r#const.name,
            Self::EnumType(r#enum) => r#enum.name,
            Self::EnumValue { .. } => unreachable!(),
            Self::PrimitiveType(_) => unreachable!(),
            Self::Local(local) => local.name,
            Self::Module(module) => module.name,
        }
    }

    pub fn get_type(&self) -> ScopeSymbolType {
        match self {
            Self::Circ(_) => ScopeSymbolType::Circ,
            Self::Const(_) => ScopeSymbolType::Const,
            Self::EnumType(_) => ScopeSymbolType::EnumType,
            Self::EnumValue { .. } => unreachable!(),
            Self::PrimitiveType(_) => ScopeSymbolType::PrimitiveType,
            Self::Local(_) => ScopeSymbolType::Local,
            Self::Module(_) => ScopeSymbolType::Module,
        }
    }
}

impl Position for ScopeSymbol {
    fn pos(&self) -> Pos {
        match self {
            Self::Circ(circ) => circ.pos,
            Self::Const(r#const) => r#const.pos,
            Self::EnumType(r#enum) => r#enum.pos,
            Self::EnumValue { .. } => unreachable!(),
            Self::PrimitiveType(_) => Pos::Builtin,
            Self::Local(local) => local.pos,
            Self::Module(module) => module.pos,
        }
    }
}

#[derive(Debug)]
pub(super) struct Scope {
    parent: Option<Rc<Scope>>,
    symbols: HashMap<IdentId, ScopeSymbol>,
}

impl Scope {
    fn _builtin_circs(_interner: &mut Interner<String>) -> [(IdentId, CircSymbol); 0] {
        []
    }

    fn _builtin_consts(interner: &mut Interner<String>) -> [(IdentId, ConstSymbol); 2] {
        let r#true = interner.intern_ref("true");
        let r#false = interner.intern_ref("false");

        [
            (
                r#true,
                ConstSymbol {
                    name: r#true,
                    r#type: ConstType::Bool,
                    value: ConstValue::TRUE,
                    pos: Pos::Builtin,
                },
            ),
            (
                r#false,
                ConstSymbol {
                    name: r#false,
                    r#type: ConstType::Bool,
                    value: ConstValue::FALSE,
                    pos: Pos::Builtin,
                },
            ),
        ]
    }

    fn _builtin_enums(_interner: &mut Interner<String>) -> [(IdentId, EnumSymbol); 0] {
        []
    }

    fn _builtin_primitive_types(interner: &mut Interner<String>) -> [(IdentId, ConstType); 2] {
        let int = interner.intern_ref("int");
        let r#bool = interner.intern_ref("bool");

        [(int, ConstType::Int), (r#bool, ConstType::Bool)]
    }

    fn _builtin_locals(_interner: &mut Interner<String>) -> [(IdentId, LocalSymbol); 0] {
        []
    }

    fn _builtin_modules(_interner: &mut Interner<String>) -> [(IdentId, ModuleSymbol); 0] {
        []
    }

    pub fn builtin(interner: &mut Interner<String>) -> Rc<Self> {
        let mut symbols = HashMap::new();
        symbols.extend(
            Self::_builtin_circs(interner)
                .map(|(name, sym)| (name, ScopeSymbol::Circ(Rc::new(sym)))),
        );

        symbols.extend(
            Self::_builtin_consts(interner).map(|(name, sym)| (name, ScopeSymbol::Const(sym))),
        );

        symbols.extend(
            Self::_builtin_enums(interner)
                .map(|(name, sym)| (name, ScopeSymbol::EnumType(Rc::new(sym)))),
        );

        symbols.extend(
            Self::_builtin_primitive_types(interner)
                .map(|(name, sym)| (name, ScopeSymbol::PrimitiveType(sym))),
        );

        symbols.extend(
            Self::_builtin_locals(interner).map(|(name, sym)| (name, ScopeSymbol::Local(sym))),
        );

        symbols.extend(
            Self::_builtin_modules(interner).map(|(name, sym)| (name, ScopeSymbol::Module(sym))),
        );

        Rc::new(Self {
            parent: None,
            symbols,
        })
    }

    pub fn child(self: &Rc<Self>) -> Self {
        Self {
            parent: Some(self.clone()),
            symbols: HashMap::new(),
        }
    }

    fn symbol_exists(&self, name: IdentId) -> bool {
        self.symbols.contains_key(&name)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.symbol_exists(name))
    }

    pub fn add_symbol(&mut self, name: IdentId, symbol: ScopeSymbol) {
        self.symbols.insert(name, symbol);
    }

    pub fn add_circ(&mut self, circ: CircSymbol) {
        let name = circ.name;
        self.add_symbol(name, ScopeSymbol::Circ(Rc::new(circ)));
    }

    pub fn add_const(&mut self, r#const: ConstSymbol) {
        let name = r#const.name;
        self.add_symbol(name, ScopeSymbol::Const(r#const));
    }

    pub fn add_module(&mut self, module: ModuleSymbol) {
        let name = module.name;
        self.add_symbol(name, ScopeSymbol::Module(module));
    }

    pub fn add_enum(&mut self, r#enum: EnumSymbol) {
        let name = r#enum.name;
        self.add_symbol(name, ScopeSymbol::EnumType(Rc::new(r#enum)));
    }

    pub fn add_local(&mut self, local: LocalSymbol) {
        let name = local.name;
        self.add_symbol(name, ScopeSymbol::Local(local));
    }

    pub fn get_symbol(&self, name: IdentId) -> Option<ScopeSymbol> {
        self.symbols
            .get(&name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_symbol(name)))
    }
}
