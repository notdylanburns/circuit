use std::collections::HashMap;

pub mod ffi;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArgValue {
    Bool(bool),
    Int(isize),
    EnumValue(String),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgType {
    Bool,
    Int,
    Enum,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinDirection {
    Input,
    Output,
    Transput,
}

#[repr(C)]
pub struct State {}

impl State {
    pub fn write_outputs(&mut self, _outputs: &[u8]) {
        todo!()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickBehavior {
    Always,
    OnChange,
    Once,
}

macro_rules! from_ffi_str {
    ($s:expr) => {
        <&str>::from_ffi(&$s)
    };
}

macro_rules! from_ffi_array {
    ($t:ty, $arr:expr, $len: expr) => {
        std::slice::from_raw_parts($arr, $len)
            .iter()
            .map(|item| unsafe { <$t>::from_ffi(item) })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    };
}

trait FromFfi {
    type Ffi;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self;
}

impl FromFfi for &'static str {
    type Ffi = *const u8;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        std::ffi::CStr::from_ptr(*from as *const i8)
            .to_str()
            .expect("invalid utf-8 in FFI string")
    }
}

#[derive(Debug)]
pub struct EnumDescriptor {
    pub name: &'static str,
    pub variants: Box<[&'static str]>,
}

impl FromFfi for EnumDescriptor {
    type Ffi = ffi::EnumDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            name: from_ffi_str!(from.name),
            variants: from_ffi_array!(&'static str, from.variants, from.variant_count),
        }
    }
}

#[derive(Debug)]
pub struct ArgDescriptor {
    pub name: &'static str,
    pub arg_type: ArgType,
}

impl FromFfi for ArgDescriptor {
    type Ffi = ffi::ArgDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            name: <&str>::from_ffi(&from.name),
            arg_type: from.arg_type,
        }
    }
}

#[derive(Debug)]
pub struct Pin {
    pub name: &'static str,
    pub width: usize,
    pub direction: PinDirection,
}

impl FromFfi for Pin {
    type Ffi = ffi::Pin;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            name: from_ffi_str!(from.name),
            width: from.width,
            direction: from.direction,
        }
    }
}

pub struct CircMeta {
    pub pins: Box<[Pin]>,
    pub tick_behaviour: TickBehavior,
}

impl FromFfi for CircMeta {
    type Ffi = ffi::CircMeta;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            pins: from_ffi_array!(Pin, from.pins, from.pin_count),
            tick_behaviour: from.tick_behaviour,
        }
    }
}

impl CircMeta {
    pub fn to_ffi(&self) -> ffi::CircMeta {
        ffi::CircMeta {
            pin_count: self.pins.len(),
            pins: self.pins.as_ptr() as *const ffi::Pin,
            tick_behaviour: self.tick_behaviour,
        }
    }
}

struct CircDescriptor {
    pub name: &'static str,
    pub args: Box<[ArgDescriptor]>,
    pub mem_size: usize,
    pub initialise: unsafe extern "C" fn(*mut (), usize, *const ArgValue),
    pub tick: unsafe extern "C" fn(*mut (), *mut State),
    pub get_meta: unsafe extern "C" fn(*mut ()) -> ffi::CircMeta,
}

impl FromFfi for CircDescriptor {
    type Ffi = ffi::CircDescriptor;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            name: from_ffi_str!(from.name),
            args: from_ffi_array!(ArgDescriptor, from.args, from.arg_count),
            mem_size: from.mem_size,
            initialise: from.initialise,
            tick: from.tick,
            get_meta: from.get_meta,
        }
    }
}

struct Library {
    pub name: &'static str,
    pub circs: Box<[CircDescriptor]>,
    pub enums: Box<[EnumDescriptor]>,
    pub libraries: Box<[Library]>,
}

impl FromFfi for Library {
    type Ffi = ffi::Library;

    unsafe fn from_ffi(from: &Self::Ffi) -> Self {
        Self {
            name: from_ffi_str!(from.name),
            circs: from_ffi_array!(CircDescriptor, from.circs, from.circ_count),
            enums: from_ffi_array!(EnumDescriptor, from.enums, from.enum_count),
            libraries: from_ffi_array!(Library, from.libraries, from.library_count),
        }
    }
}

pub trait Circ {
    fn new(args: Args) -> Self;
    fn tick_internal(&mut self, state: &mut State);
    fn get_meta_internal(&self) -> CircMeta;
}

pub trait CircDesc<C: Circ> {
    unsafe extern "C" fn initialise(this: *mut (), argc: usize, argv: *const ArgValue) {
        let this = std::mem::transmute::<*mut (), *mut C>(this);
        *this = C::new(std::slice::from_raw_parts(argv, argc));
    }

    unsafe extern "C" fn tick(this: *mut (), state: *mut State) {
        let this = std::mem::transmute::<*mut (), *mut C>(this);
        C::tick_internal(&mut *this, &mut *state);
    }

    unsafe extern "C" fn get_meta(this: *mut ()) -> ffi::CircMeta {
        let this = std::mem::transmute::<*mut (), *const C>(this);
        C::get_meta_internal(&*this).to_ffi()
    }
}

impl<T: Circ> CircDesc<T> for T {}

pub type Args<'a> = &'a [ArgValue];

pub fn get_build_consts(
    argc: usize,
    argv: *const ffi::BuildConst,
) -> HashMap<&'static str, ffi::BuildConstValue> {
    let argv = unsafe { std::slice::from_raw_parts(argv, argc) };
    argv.iter()
        .map(|arg| {
            let name = unsafe { <&str>::from_ffi(&arg.name) };
            (name, arg.value)
        })
        .collect::<HashMap<_, _>>()
}
