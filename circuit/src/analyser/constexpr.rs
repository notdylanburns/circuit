use crate::loader::ModuleId;
use crate::tokeniser::IdentId;

#[derive(Debug, Hash, Eq, Copy, Clone)]
pub enum ConstType {
    Unknown,
    Int,
    Bool,
    Enum {
        module_id: ModuleId,
        enum_name: IdentId,
    },
    Range,
}

impl ConstType {
    pub fn is_known(&self) -> bool {
        !self.is_unknown()
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Int => "int",
            Self::Bool => "bool",
            Self::Enum { .. } => "enum",
            Self::Range => "range",
        }
    }
}

impl PartialEq for ConstType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown, _) => true,
            (_, Self::Unknown) => true,
            (Self::Int, Self::Int) => true,
            (Self::Bool, Self::Bool) => true,
            (
                Self::Enum {
                    module_id: m1,
                    enum_name: n1,
                },
                Self::Enum {
                    module_id: m2,
                    enum_name: n2,
                },
            ) => m1 == m2 && n1 == n2,
            _ => false,
        }
    }
}

impl From<circuit_extlib::ConstType> for ConstType {
    fn from(value: circuit_extlib::ConstType) -> Self {
        match value {
            circuit_extlib::ConstType::Int => ConstType::Int,
            circuit_extlib::ConstType::Bool => ConstType::Bool,
            circuit_extlib::ConstType::Enum => todo!(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub enum ConstValue {
    Unknown,
    Int(isize),
    Bool(bool),
    Enum {
        module_id: ModuleId,
        enum_name: IdentId,
        variant: IdentId,
    },
    Range(Option<isize>, Option<isize>),
}

impl ConstValue {
    pub const TRUE: Self = Self::Bool(true);
    pub const FALSE: Self = Self::Bool(false);

    pub fn get_type(&self) -> ConstType {
        match self {
            Self::Unknown => ConstType::Unknown,
            Self::Int(_) => ConstType::Int,
            Self::Bool(_) => ConstType::Bool,
            Self::Enum {
                module_id,
                enum_name,
                ..
            } => ConstType::Enum {
                module_id: *module_id,
                enum_name: *enum_name,
            },
            Self::Range(_, _) => ConstType::Range,
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn is_known(&self) -> bool {
        !self.is_unknown()
    }

    pub fn from_extracted_const(value: &circuit_extlib::rust::ExtractedConst) -> Option<Self> {
        match value {
            circuit_extlib::rust::ExtractedConst::None => None,
            circuit_extlib::rust::ExtractedConst::Bool(b) => Some(Self::Bool(*b)),
            circuit_extlib::rust::ExtractedConst::EnumValue(_) => todo!(),
            circuit_extlib::rust::ExtractedConst::Int(i) => Some(Self::Int(*i)),
        }
    }
}
