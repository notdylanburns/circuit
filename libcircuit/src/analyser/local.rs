use crate::{analyser::CircSignature, util::difference, IdentId};

use super::scope::LocalSymbol;
use super::CircId;
use crate::ast::PinDirection;
use std::rc::Rc;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum LocalType {
    Unknown,
    Pin {
        direction: PinDirection,
        width: usize,
    },
    Circ(CircId),
    Array(CircId, usize),
}

impl LocalType {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn is_pin(&self) -> bool {
        matches!(self, Self::Pin(_, _))
    }

    pub fn is_circ(&self) -> bool {
        matches!(self, Self::Circ(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_, _))
    }

    pub fn as_array(&self, length: usize) -> Self {
        match self {
            Self::Circ(id) => Self::Array(*id, length),
            _ => unreachable!("cannot convert to array"),
        }
    }
}

impl std::fmt::Display for LocalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Pin(direction, width) => write!(f, "pin({direction:?}, {width})"),
            Self::Circ(id) => write!(f, "circ({id})"),
            Self::Array(id, length) => write!(f, "array({id}, {length})"),
            Self::Identifier => write!(f, "identifier"),
        }
    }
}

pub enum LocalValue {
    Unknown,
    Pin(PinDirection, usize),
    Circ(CircId),
    Array(CircId, usize),
    Identifier(IdentId),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct Local {
    pub r#type: LocalType,
    pub parent: Option<Rc<Self>>,
    pub range: Option<(usize, usize)>,
    pub name: Option<IdentId>,
}

impl Local {
    pub const UNKNOWN: Self = Self {
        r#type: LocalType::Unknown,
        parent: None,
        range: None,
        name: None,
    };

    pub const fn pin(direction: PinDirection, width: usize, name: IdentId) -> Self {
        Self {
            r#type: LocalType::Pin(direction, width),
            parent: None,
            range: None,
            name: Some(name),
        }
    }

    pub const fn circ(id: CircId, name: IdentId) -> Self {
        Self {
            r#type: LocalType::Circ(id),
            parent: None,
            range: None,
            name: Some(name),
        }
    }

    pub const fn array(id: CircId, length: usize, name: IdentId) -> Self {
        Self {
            r#type: LocalType::Array(id, length),
            parent: None,
            range: None,
            name: Some(name),
        }
    }

    pub const fn identifier(name: IdentId) -> Self {
        Self {
            r#type: LocalType::Identifier,
            parent: None,
            range: None,
            name: Some(name),
        }
    }

    pub fn as_enum(&self) -> LocalValue {
        match self.r#type {
            LocalType::Unknown => LocalValue::Unknown,
            LocalType::Pin(direction, width) => LocalValue::Pin(direction, width),
            LocalType::Circ(id) => LocalValue::Circ(id),
            LocalType::Array(id, length) => LocalValue::Array(id, length),
            LocalType::Identifier => LocalValue::Identifier(self.name()),
        }
    }

    pub fn rc(self) -> Rc<Self> {
        Rc::new(self)
    }

    pub fn get_type(&self) -> LocalType {
        self.r#type
    }

    pub fn get_index(self: &Rc<Self>, index: usize) -> Self {
        match self.r#type {
            LocalType::Array(circ_id, _) => Self {
                r#type: LocalType::Circ(circ_id),
                parent: Some(self.clone()),
                range: Some((index, index)),
                name: None,
            },
            LocalType::Pin(direction, _) => Self {
                r#type: LocalType::Pin(direction, 1),
                parent: Some(self.clone()),
                range: Some((index, index)),
                name: None,
            },
            _ => unreachable!("called get_index on {:?}", self.r#type),
        }
    }

    pub fn get_range(self: &Rc<Self>, start: usize, end: usize) -> Self {
        if let LocalType::Pin(direction, _) = self.r#type {
            Self {
                r#type: LocalType::Pin(direction, difference!(start, end)),
                parent: Some(self.clone()),
                range: Some((start, end)),
                name: None,
            }
        } else {
            unreachable!("called get_range on {:?}", self.r#type)
        }
    }

    pub fn name(&self) -> IdentId {
        self.name.unwrap_or_else(|| unreachable!())
    }

    pub fn get_child(
        self: &Rc<Self>,
        name: IdentId,
        direction: PinDirection,
        width: usize,
    ) -> Self {
        if let LocalType::Circ(_) = self.r#type {
            Self {
                r#type: LocalType::Pin(direction, width),
                parent: Some(self.clone()),
                range: None,
                name: Some(name),
            }
        } else {
            unreachable!("called get_child on {:?}", self.r#type)
        }
    }

    pub fn from_symbol(symbol: &LocalSymbol) -> Self {
        match symbol.r#type {
            LocalType::Pin(direction, width) => Self::pin(direction, width, symbol.name),
            LocalType::Circ(id) => Self::circ(id, symbol.name),
            LocalType::Array(id, length) => Self::array(id, length, symbol.name),
            LocalType::Unknown => Self::UNKNOWN,
            LocalType::Identifier => unreachable!(),
        }
    }
}
