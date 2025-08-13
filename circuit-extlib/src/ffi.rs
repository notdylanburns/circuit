use crate::{ArgType, ArgValue, PinDirection, State, TickBehavior};

#[repr(C)]
#[derive(Debug)]
pub struct EnumDescriptor {
    pub name: *const u8,
    pub variant_count: usize,
    pub variants: *const *const u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArgDescriptor {
    pub name: *const u8,
    pub arg_type: ArgType,
}

#[repr(C)]
#[derive(Debug)]
pub struct Pin {
    pub name: *const u8,
    pub width: usize,
    pub direction: PinDirection,
}

#[repr(C)]
#[derive(Debug)]
pub struct CircMeta {
    pub pin_count: usize,
    pub pins: *const Pin,
    pub tick_behaviour: TickBehavior,
}

#[repr(C)]
#[derive(Debug)]
pub struct CircDescriptor {
    pub name: *const u8,
    pub arg_count: usize,
    pub args: *const ArgDescriptor,
    pub mem_size: usize,
    pub initialise: unsafe extern "C" fn(*mut (), usize, *const ArgValue),
    pub tick: unsafe extern "C" fn(*mut (), *mut State),
    pub get_meta: unsafe extern "C" fn(*mut ()) -> CircMeta,
}

#[repr(C)]
#[derive(Debug)]
pub struct Library {
    pub name: *const u8,
    pub circ_count: usize,
    pub circs: *const CircDescriptor,
    pub enum_count: usize,
    pub enums: *const EnumDescriptor,
    pub library_count: usize,
    pub libraries: *const Library,
}

unsafe impl Sync for Library {}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildConstValue {
    Int(isize),
    Bool(bool),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildConst {
    pub name: *const u8,
    pub value: BuildConstValue,
}

#[macro_export]
macro_rules! cstr {
    ($s:literal) => {
        concat!($s, "\0").as_ptr() as *const u8
    };
}

#[macro_export]
macro_rules! library {
    (
        name: $name:literal,
        circs: $circs:expr,
        enums: $enums:expr,
        libraries: $libraries:expr
        $(,)?
    ) => {
        $crate::ffi::Library {
            name: $crate::cstr!($name),
            circ_count: ($circs).len(),
            circs: ($circs).as_ptr(),
            enum_count: ($enums).len(),
            enums: ($enums).as_ptr(),
            library_count: ($libraries).len(),
            libraries: ($libraries).as_ptr(),
        }
    };
}

#[macro_export]
macro_rules! circ {
    (
        name: $circ_name:literal,
        args: $args:expr,
        mem_size: $mem_size:expr,
        initialise: $init_fn:expr,
        tick: $tick_fn:expr,
        get_meta: $get_meta_fn:expr
        $(,)?
    ) => {
        $crate::ffi::CircDescriptor {
            name: $crate::cstr!($circ_name),
            arg_count: ($args).len(),
            args: ($args).as_ptr(),
            mem_size: $mem_size,
            initialise: $init_fn,
            tick: $tick_fn,
            get_meta: $get_meta_fn,
        }
    };
}

#[macro_export]
macro_rules! arg {
    (
        arg_type: $arg_type:expr
        $(,)?
    ) => {
        $crate::ffi::ArgDescriptor {
            name: std::ptr::null(),
            arg_type: $arg_type,
        }
    };
    (
        name: $name:literal,
        arg_type: $arg_type:expr
        $(,)?
    ) => {
        $crate::ffi::ArgDescriptor {
            name: $crate::cstr!($name),
            arg_type: $arg_type,
        }
    };
}
