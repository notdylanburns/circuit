use std::rc::Rc;

macro_rules! from_ffi_str {
    ($s:expr) => {
        <Box<str>>::from_ffi(&$s)
    };
}

macro_rules! from_ffi_array {
    ($t:ty, $arr:expr, $len: expr) => {{
        std::slice::from_raw_parts($arr, $len)
            .iter()
            .map(|item| unsafe { <$t>::from_ffi(item) })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }};
}

pub trait FromFfi {
    type Ffi;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self;
}

impl FromFfi for Box<str> {
    type Ffi = *const u8;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Box::from(
            std::ffi::CStr::from_ptr(*from as *const i8)
                .to_str()
                .expect("invalid utf-8 in FFI string"),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefinedAt {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl FromFfi for DefinedAt {
    type Ffi = circuit_extlib::DefinedAt;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            file: std::ffi::CStr::from_ptr(from.file as *const i8)
                .to_str()
                .expect("invalid utf-8 in FFI string")
                .into(),
            line: from.line,
            column: from.column,
        }
    }
}

#[derive(Debug)]
pub struct EnumDescriptor {
    pub defined_at: DefinedAt,
    pub name: Box<str>,
    pub variants: Box<[Box<str>]>,
}

impl FromFfi for EnumDescriptor {
    type Ffi = circuit_extlib::EnumDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            defined_at: DefinedAt::from_ffi(&from.defined_at),
            name: from_ffi_str!(from.name),
            variants: from_ffi_array!(Box<str>, from.variants, from.variant_count),
        }
    }
}

#[derive(Debug)]
pub struct ArgDescriptor {
    pub defined_at: DefinedAt,
    pub name: Box<str>,
    pub arg_type: circuit_extlib::ConstType,
    pub default: circuit_extlib::rust::ExtractedConst,
}

impl FromFfi for ArgDescriptor {
    type Ffi = circuit_extlib::ArgDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            defined_at: DefinedAt::from_ffi(&from.defined_at),
            name: from_ffi_str!(from.name),
            arg_type: from.arg_type,
            default: circuit_extlib::rust::extract_const(&from.default),
        }
    }
}

#[derive(Debug)]
pub struct Pin {
    pub defined_at: DefinedAt,
    pub name: Box<str>,
    pub width: usize,
    pub direction: circuit_extlib::PinDirection,
}

impl FromFfi for Pin {
    type Ffi = circuit_extlib::Pin;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            defined_at: DefinedAt::from_ffi(&from.defined_at),
            name: from_ffi_str!(from.name),
            width: from.width,
            direction: from.direction,
        }
    }
}

pub struct CircMeta {
    pub mem_size: usize,
    pub pins: Box<[Pin]>,
    pub tick_behaviour: circuit_extlib::TickBehaviour,
}

impl FromFfi for CircMeta {
    type Ffi = circuit_extlib::CircMeta;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            mem_size: from.mem_size,
            pins: from_ffi_array!(Pin, from.pins, from.pin_count),
            tick_behaviour: from.tick_behaviour,
        }
    }
}

pub struct CircDescriptor {
    pub defined_at: DefinedAt,
    pub name: Box<str>,
    pub args: Box<[ArgDescriptor]>,
    pub initialise: circuit_extlib::InitialiserFn,
    pub tick: circuit_extlib::TickFn,
    pub get_meta: circuit_extlib::GetMetaFn,
}

impl FromFfi for CircDescriptor {
    type Ffi = circuit_extlib::CircDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            defined_at: DefinedAt::from_ffi(&from.defined_at),
            name: from_ffi_str!(from.name),
            args: from_ffi_array!(ArgDescriptor, from.args, from.arg_count),
            initialise: from.initialise,
            tick: from.tick,
            get_meta: from.get_meta,
        }
    }
}

pub struct Library {
    pub defined_at: DefinedAt,
    pub name: Box<str>,
    pub circs: Box<[CircDescriptor]>,
    pub enums: Box<[EnumDescriptor]>,
    pub libraries: Box<[Library]>,
}

impl FromFfi for Library {
    type Ffi = circuit_extlib::Library;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            defined_at: DefinedAt::from_ffi(&from.defined_at),
            name: from_ffi_str!(from.name),
            circs: from_ffi_array!(CircDescriptor, from.circs, from.circ_count),
            enums: from_ffi_array!(EnumDescriptor, from.enums, from.enum_count),
            libraries: from_ffi_array!(Library, from.libraries, from.library_count),
        }
    }
}

pub type InitFn =
    unsafe extern "C" fn(usize, *const circuit_extlib::BuildConst) -> circuit_extlib::InitResult;
