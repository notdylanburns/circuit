use circuit_extlib::rust::*;
use circuit_extlib::*;
use std::collections::HashMap;

struct Level<const S: u8> {
    width: usize,
}

impl<const S: u8> Level<S> {
    const STATE: State = match S {
        Z => State::Z,
        L => State::L,
        H => State::H,
        E => State::E,
        _ => panic!("invalid state for Level"),
    };
}

struct LevelArgs {
    width: usize,
}

impl From<Args<'_>> for LevelArgs {
    fn from(args: Args) -> Self {
        use ExtractedConst::*;

        match args[..] {
            [Int(width)] => Self {
                width: width as usize,
            },
            _ => unreachable!("invalid args for Level"),
        }
    }
}

impl<const S: u8> Circ for Level<S> {
    fn new(args: Args) -> Self {
        let args = LevelArgs::from(args);

        Self { width: args.width }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        let outputs = vec![Self::STATE; self.width];
        state.write_outputs(&outputs);
    }

    fn get_meta_internal(args: Args) -> CircMeta {
        let args = LevelArgs::from(args);

        circ_meta! {
            mem_size: std::mem::size_of::<Self>(),
            pins: @[
                Output "q" [args.width],
            ],
            tick_behaviour: TickBehaviour::Once,
        }
    }
}

struct Probe {
    width: usize,
}

impl Circ for Probe {
    fn new(args: Args) -> Self {
        dbg!(args);
        let args = LevelArgs::from(args);

        Self { width: args.width }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        // dbg!(self.width);
        // // dbg!(state);
        // dbg!(std::ptr::null::<usize>());
        // let ptr = 0x00000000004040c8 as *const u8;
        // dbg!(ptr);
        let inputs = state.read_inputs(self.width);

        println!("probe: ticks={} {:?}", state.rt.ticks, inputs);
    }

    fn get_meta_internal(args: Args) -> CircMeta {
        let args = LevelArgs::from(args);

        dbg!(std::mem::align_of::<Self>());

        circ_meta! {
            mem_size: std::mem::size_of::<Self>(),
            pins: @[
                Input "a"[args.width],
            ],
            tick_behaviour: TickBehaviour::Always
        }
    }
}

const Z: u8 = 0b00;
const L: u8 = 0b01;
const H: u8 = 0b10;
const E: u8 = 0b11;

fn get_circs(_: &HashMap<&'static str, ConstValue>) -> &'static [CircDescriptor] {
    sa![
        circ! {
            name: "Z",
            args: sa! [
                arg! {
                    name: "width",
                    arg_type: ConstType::Int,
                    default: ConstValue::from(1),
                }
            ],
            initialise: Level::<Z>::initialise,
            tick: Level::<Z>::tick,
            get_meta: Level::<Z>::get_meta,
        },
        circ! {
            name: "L",
            args: sa! [
                arg! {
                    name: "width",
                    arg_type: ConstType::Int,
                    default: ConstValue::from(1),
                }
            ],
            initialise: Level::<L>::initialise,
            tick: Level::<L>::tick,
            get_meta: Level::<L>::get_meta,
        },
        circ! {
            name: "H",
            args: sa! [
                arg! {
                    name: "width",
                    arg_type: ConstType::Int,
                    default: ConstValue::from(1),
                }
            ],
            initialise: Level::<H>::initialise,
            tick: Level::<H>::tick,
            get_meta: Level::<H>::get_meta,
        },
        circ! {
            name: "E",
            args: sa! [
                arg! {
                    name: "width",
                    arg_type: ConstType::Int,
                    default: ConstValue::from(1),
                }
            ],
            initialise: Level::<E>::initialise,
            tick: Level::<E>::tick,
            get_meta: Level::<E>::get_meta,
        },
        circ! {
            name: "Probe",
            args: sa! [
                arg! {
                    name: "width",
                    arg_type: ConstType::Int,
                    default: ConstValue::from(1),
                }
            ],
            initialise: Probe::initialise,
            tick: Probe::tick,
            get_meta: Probe::get_meta,
        }
    ]
}

fn get_enums(_: &HashMap<&'static str, ConstValue>) -> &'static [EnumDescriptor] {
    sa![]
}

fn get_libraries(_: &HashMap<&'static str, ConstValue>) -> &'static [Library] {
    sa![]
}

#[no_mangle]
pub extern "C" fn init(argc: usize, argv: *const BuildConst) -> InitResult {
    let build_consts = get_build_consts(argc, argv);

    let library = library! {
        name: "MyLib",
        circs: get_circs(&build_consts),
        enums: get_enums(&build_consts),
        libraries: get_libraries(&build_consts),
    };

    InitResult {
        library,
        error: std::ptr::null(),
    }
}

// #[repr(C)]
// #[derive(Debug)]
// pub struct T {
//     pub name: *const u8,
//     pub arg_count: usize,
//     pub args: *const ArgDescriptor,
//     pub initialise: InitialiserFn,
//     pub tick: TickFn,
//     pub get_meta: GetMetaFn,
// }

// unsafe impl Sync for T {}

// #[no_mangle]
// pub static CIRC: T = T {
//     name: cstr!("hello"),
//     arg_count: 0,
//     args: vec![].as_ptr(),
//     initialise: Level::<Z>::initialise,
//     tick: Level::<Z>::tick,
//     get_meta: Level::<Z>::get_meta,
// };
