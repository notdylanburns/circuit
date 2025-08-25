#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstType {
    Bool,
    Int,
    Enum,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstValueType {
    Bool,
    Int,
    Enum,
    None,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union ConstValueInner {
    pub none: u8,
    pub bool: bool,
    pub int: isize,
    pub enum_value: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConstValue {
    pub t: ConstValueType,
    pub v: ConstValueInner,
}

impl ConstValue {
    pub const NONE: Self = Self {
        t: ConstValueType::None,
        v: ConstValueInner { none: 0 },
    };
}

impl From<bool> for ConstValue {
    fn from(value: bool) -> Self {
        Self {
            t: ConstValueType::Bool,
            v: ConstValueInner { bool: value },
        }
    }
}

impl From<isize> for ConstValue {
    fn from(value: isize) -> Self {
        Self {
            t: ConstValueType::Int,
            v: ConstValueInner { int: value },
        }
    }
}

impl std::fmt::Debug for ConstValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.t {
            ConstValueType::Bool => write!(f, "ConstValue::Bool({})", unsafe { self.v.bool }),
            ConstValueType::Int => write!(f, "ConstValue::Int({})", unsafe { self.v.int }),
            ConstValueType::Enum => write!(f, "ConstValue::Enum()"),
            ConstValueType::None => write!(f, "ConstValue::None"),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinDirection {
    Input,
    Output,
    Transput,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickBehaviour {
    Always,
    OnChange,
    Once,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DefinedAt {
    pub file: *const u8,
    pub line: u32,
    pub column: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct EnumDescriptor {
    pub defined_at: DefinedAt,
    pub name: *const u8,
    pub variant_count: usize,
    pub variants: *const *const u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct ArgDescriptor {
    pub defined_at: DefinedAt,
    pub name: *const u8,
    pub arg_type: ConstType,
    pub default: ConstValue,
}

#[repr(C)]
#[derive(Debug)]
pub struct Pin {
    pub defined_at: DefinedAt,
    pub name: *const u8,
    pub width: usize,
    pub direction: PinDirection,
}

#[repr(C)]
#[derive(Debug)]
pub struct CircMeta {
    pub error: *const u8,
    pub mem_size: usize,
    pub pin_count: usize,
    pub pins: *const Pin,
    pub tick_behaviour: TickBehaviour,
}

#[repr(C)]
#[derive(Debug)]
pub struct RuntimeState {
    pub ticks: usize,
}

pub type InitialiserFn = unsafe extern "C" fn(*mut (), usize, *const ConstValue);
pub type TickFn = unsafe extern "C" fn(*mut (), *const RuntimeState, *const usize, *mut usize);
pub type GetMetaFn = unsafe extern "C" fn(usize, *const ConstValue) -> CircMeta;

#[repr(C)]
#[derive(Debug)]
pub struct CircDescriptor {
    pub defined_at: DefinedAt,
    pub name: *const u8,
    pub arg_count: usize,
    pub args: *const ArgDescriptor,
    pub initialise: InitialiserFn,
    pub tick: TickFn,
    pub get_meta: GetMetaFn,
}

#[repr(C)]
#[derive(Debug)]
pub struct Library {
    pub defined_at: DefinedAt,
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
#[derive(Debug, Clone, Copy)]
pub struct BuildConst {
    pub name: *const u8,
    pub value: ConstValue,
}

#[repr(C)]
#[derive(Debug)]
pub struct InitResult {
    pub library: Library,
    pub error: *const u8,
}

#[macro_export]
macro_rules! defined_at {
    () => {
        DefinedAt {
            file: $crate::cstr!(file!()),
            line: line!(),
            column: column!(),
        }
    };
}

#[macro_export]
macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0").as_ptr()
    };
}

#[macro_export]
macro_rules! sa {
    ($($item:expr),*) => {
        Box::leak(
            Box::new([
                $($item,)*
            ])
        )
    };
    ($($item:expr),*,) => {
        $crate::sa!($($item),*)
    }
}

#[macro_export]
macro_rules! library {
    (
        name: $name:expr,
        circs: $circs:expr,
        enums: $enums:expr,
        libraries: $libraries:expr
        $(,)?
    ) => {{
        let circs = $circs;
        let enums = $enums;
        let libraries = $libraries;

        $crate::Library {
            defined_at: $crate::defined_at!(),
            name: $crate::cstr!($name),
            circ_count: circs.len(),
            circs: circs.as_ptr(),
            enum_count: enums.len(),
            enums: enums.as_ptr(),
            library_count: libraries.len(),
            libraries: libraries.as_ptr(),
        }
    }};
}

#[macro_export]
macro_rules! circ {
    (
        name: $circ_name:expr,
        args: $args:expr,
        initialise: $init_fn:expr,
        tick: $tick_fn:expr,
        get_meta: $get_meta_fn:expr
        $(,)?
    ) => {{
        let args = $args;

        $crate::CircDescriptor {
            defined_at: $crate::defined_at!(),
            name: $crate::cstr!($circ_name),
            arg_count: args.len(),
            args: args.as_ptr(),
            initialise: $init_fn,
            tick: $tick_fn,
            get_meta: $get_meta_fn,
        }
    }}; // (
        //     name: $circ_name:expr,
        //     args: @[
        //         $($t:ident $($name:ident)? $(= $default:expr)?),*
        //     ],
        //     initialise: $init_fn:expr,
        //     tick: $tick_fn:expr,
        //     get_meta: $get_meta_fn:expr
        //     $(,)?
        // ) => {
        //     $crate::circ! {
        //         name: name,
        //         args: sa![
        //             $(
        //                 $crate::arg! {
        //                     $(name: $name,)?
        //                     arg_type: $crate::ArgType::$t,
        //                     $(default: $crate::ArgValue {
        //                         t: $crate::ArgValueType::$t,
        //                         v: $crate::ArgValueInner {
        //                             $(
        //                                 $name: $default,
        //                             )?
        //                             none: 0,
        //                         },
        //                     })?
        //                 }
        //             )*
        //         ]
        //     }
        // };
}

#[macro_export]
macro_rules! arg {
    (
        name: $name:expr,
        arg_type: $arg_type:expr
        $(,)?
    ) => {
        $crate::ArgDescriptor {
            defined_at: $crate::defined_at!(),
            name: $crate::cstr!($name),
            arg_type: $arg_type,
            default: $crate::ConstValue::NONE,
        }
    };
    (
        name: $name:expr,
        arg_type: $arg_type:expr,
        default: $default:expr
        $(,)?
    ) => {
        $crate::ArgDescriptor {
            defined_at: $crate::defined_at!(),
            name: $crate::cstr!($name),
            arg_type: $arg_type,
            default: $default,
        }
    };
    ($t:ident $($name:ident)? $(= $default:expr)?) => {
        $crate::arg! {
            $(name: stringify!($name),)?
            arg_type: $crate::ConstType::$t,
            $(default: $crate::ConstValue::from($default),)?
        }
    };
}

#[macro_export]
macro_rules! pin {
    (
        name: $name:expr,
        direction: $direction:expr,
        width: $width:expr
        $(,)?
    ) => {
        $crate::Pin {
            defined_at: $crate::defined_at!(),
            name: $crate::cstr!($name),
            width: $width,
            direction: $direction,
        }
    };
    (
        $direction:ident $name:ident[$width:expr]
    ) => {
        $crate::pin! {
            name: stringify!($name),
            direction: $crate::PinDirection::$direction,
            width: $width,
        }
    };
}

#[macro_export]
macro_rules! circ_meta {
    (
        mem_size: $mem_size:expr,
        pins: $pins:expr,
        tick_behaviour: $tick_behaviour:expr
        $(,)?
    ) => {{
        let pins = $pins;

        $crate::CircMeta {
            error: std::ptr::null(),
            mem_size: $mem_size,
            pin_count: pins.len(),
            pins: pins.as_ptr(),
            tick_behaviour: $tick_behaviour,
        }
    }};
    (
        mem_size: $mem_size:expr,
        pins: @[
            $(
                $direction:ident $name:literal [$w:expr]
            ),*
            $(,)?
        ],
        tick_behaviour: $tick_behaviour:expr
        $(,)?
    ) => {
        $crate::circ_meta!(
            mem_size: $mem_size,
            pins: sa![
                $(
                    $crate::Pin {
                        defined_at: $crate::defined_at!(),
                        name: $crate::cstr!($name),
                        width: $w,
                        direction: $crate::PinDirection::$direction,
                    },
                )*
            ],
            tick_behaviour: $tick_behaviour,
        )
    }
}

// Rust only abstractions

pub mod rust {
    use super::{BuildConst, CircMeta, ConstValue, ConstValueType, RuntimeState, TickBehaviour};
    use std::collections::HashMap;

    pub type Args<'a> = &'a [ExtractedConst];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum State {
        Z,
        L,
        H,
        E,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SimState {
        pub rt: &'static RuntimeState,
        inputs: *const usize,
        outputs: *mut usize,
    }

    impl SimState {
        fn get_shift(index: usize) -> (usize, usize) {
            let bit_index = index * 2;
            let word = index / (std::mem::size_of::<usize>() * 8);
            let index = bit_index % (std::mem::size_of::<usize>() * 8);

            (word, index)
        }

        fn get_read_mask(index: usize) -> (usize, usize, usize) {
            let (word, shift) = Self::get_shift(index);

            (word, shift, 0b11 << shift)
        }

        fn get_set_mask(index: usize, v: State) -> (usize, usize) {
            let (word, shift) = Self::get_shift(index);
            let v = match v {
                State::Z => 0,
                State::L => 1,
                State::H => 2,
                State::E => 3,
            };

            (word, v << shift)
        }

        fn get_clear_mask(index: usize) -> (usize, usize) {
            let (word, shift) = Self::get_shift(index);
            let mask = 3 << shift; // Clear the two bits for this index
            (word, !mask)
        }

        fn get_masks(index: usize, value: State) -> (usize, usize, usize) {
            let (word, clear_mask) = Self::get_clear_mask(index);
            let (word2, set_mask) = Self::get_set_mask(index, value);
            assert_eq!(word, word2);

            (word, clear_mask, set_mask)
        }

        pub fn read_input(&self, index: usize) -> State {
            let (word, shift, mask) = Self::get_read_mask(index);

            let v = unsafe {
                let input_ptr = self.inputs.add(word);
                (*input_ptr & mask) >> shift
            };

            match v {
                0 => State::Z,
                1 => State::L,
                2 => State::H,
                3 => State::E,
                _ => unreachable!(),
            }
        }

        pub fn read_inputs(&self, count: usize) -> Vec<State> {
            (0..count).map(|i| self.read_input(i)).collect()
        }

        pub fn write_output(&mut self, index: usize, value: State) {
            let (word, clear, set) = Self::get_masks(index, value);

            unsafe {
                let output_ptr = self.outputs.add(word);
                *output_ptr &= clear;
                *output_ptr |= set;
            }
        }

        pub fn write_outputs(&mut self, outputs: &[State]) {
            for (i, v) in outputs.iter().enumerate() {
                self.write_output(i, *v);
            }
        }
    }

    pub trait Circ {
        fn new(args: Args) -> Self;
        fn tick_internal(&mut self, state: &mut SimState);
        fn get_meta_internal(args: Args) -> Result<CircMeta, &'static str>;
    }

    pub trait CircDesc<C: Circ> {
        /// # Safety
        ///
        /// The caller must ensure that `this` is a valid pointer to a block of size
        /// `Self::get_meta().mem_size`, and that `argv` is a valid pointer to an array of
        /// `ConstValue` of length `argc`. The values in `argv` must be valid for the circuit.
        unsafe extern "C" fn initialise(this: *mut (), argc: usize, argv: *const ConstValue) {
            let this = this.cast();
            let args = extract_consts(std::slice::from_raw_parts(argv, argc)).collect::<Vec<_>>();
            *this = C::new(&args);
        }

        /// # Safety
        ///
        /// The caller must ensure that `this` is a valid pointer to a block of size
        /// Self::get_meta().mem_size, and that it has been initialised with a call to
        /// Self::initialise. `runtime_state` must be a valid, non-null pointer to a
        /// `RuntimeState` struct. `inputs` and `outputs` must point to a slice of `usize`,
        /// long enough to hold at least the amount of bits to represent the Input/Output
        /// pins returned by Self::get_meta(), or be `null` in case there are no pins of that type.
        unsafe extern "C" fn tick(
            this: *mut (),
            runtime_state: *const RuntimeState,
            inputs: *const usize,
            outputs: *mut usize,
        ) {
            let this = this.cast();

            C::tick_internal(
                &mut *this,
                &mut SimState {
                    rt: &*runtime_state,
                    inputs,
                    outputs,
                },
            );
        }

        /// # Safety
        ///
        /// The caller must ensure that `argv` is a valid pointer to an array of `ConstValue`
        /// of length `argc`, and that the values in `argv` are valid for the circuit.
        unsafe extern "C" fn get_meta(argc: usize, argv: *const ConstValue) -> CircMeta {
            let args = extract_consts(std::slice::from_raw_parts(argv, argc)).collect::<Vec<_>>();
            match C::get_meta_internal(&args) {
                Ok(meta) => meta,
                Err(e) => CircMeta {
                    error: e.as_ptr(),
                    mem_size: 0,
                    pin_count: 0,
                    pins: std::ptr::null(),
                    tick_behaviour: TickBehaviour::Always,
                },
            }
        }
    }

    impl<T: Circ> CircDesc<T> for T {}

    pub type BuildConsts = HashMap<&'static str, ConstValue>;

    pub fn get_build_consts(argc: usize, argv: *const BuildConst) -> BuildConsts {
        let argv = unsafe { std::slice::from_raw_parts(argv, argc) };
        argv.iter()
            .map(|arg| unsafe {
                let name = std::ffi::CStr::from_ptr(*arg.name as *const i8)
                    .to_str()
                    .expect("invalid utf-8 in FFI string");
                (name, arg.value)
            })
            .collect::<HashMap<_, _>>()
    }

    #[derive(Debug, Clone)]
    pub enum ExtractedConst {
        None,
        Int(isize),
        Bool(bool),
        EnumValue(String),
    }

    pub fn extract_const(c: &ConstValue) -> ExtractedConst {
        match c {
            ConstValue {
                t: ConstValueType::None,
                ..
            } => ExtractedConst::None,
            ConstValue {
                t: ConstValueType::Int,
                v,
            } => ExtractedConst::Int(unsafe { v.int }),
            ConstValue {
                t: ConstValueType::Bool,
                v,
            } => ExtractedConst::Bool(unsafe { v.bool }),
            ConstValue {
                t: ConstValueType::Enum,
                v,
            } => ExtractedConst::EnumValue(todo!()),
        }
    }

    pub fn extract_consts(c: &[ConstValue]) -> impl Iterator<Item = ExtractedConst> + '_ {
        c.iter().map(extract_const)
    }
}
