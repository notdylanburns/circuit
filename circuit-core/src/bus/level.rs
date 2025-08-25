use circuit_extlib::rust::*;
use circuit_extlib::*;

struct ArgParser {
    width: usize,
}

impl ArgParser {
    fn parse(args: Args) -> Result<Self, &'static str> {
        use ExtractedConst::*;

        let &[Int(width)] = args else {
            unreachable!("invalid arg types")
        };

        if width <= 0 {
            return Err("width must be at least 1");
        }

        Ok(Self {
            width: width as usize,
        })
    }
}

pub(super) struct Level<const S: u8> {
    width: usize,
}

pub(super) const Z: u8 = 0b00;
pub(super) const L: u8 = 0b01;
pub(super) const H: u8 = 0b10;
pub(super) const E: u8 = 0b11;

impl<const S: u8> Level<S> {
    const STATE: State = match S {
        Z => State::Z,
        L => State::L,
        H => State::H,
        E => State::E,
        _ => panic!("invalid state for Level"),
    };
}

impl<const S: u8> Circ for Level<S> {
    fn new(args: Args) -> Self {
        let args = ArgParser::parse(args).unwrap();

        Self { width: args.width }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        let outputs = vec![Self::STATE; self.width];
        state.write_outputs(&outputs);
    }

    fn get_meta_internal(args: Args) -> Result<CircMeta, &'static str> {
        let args = ArgParser::parse(args)?;

        if args.width == 0 {
            Err("width cannot be 0")
        } else {
            Ok(circ_meta! {
                mem_size: std::mem::size_of::<Self>(),
                pins: sa![
                    pin!(Output q[args.width]),
                ],
                tick_behaviour: TickBehaviour::Once,
            })
        }
    }
}

macro_rules! level {
    ($l:ident) => {
        circ! {
            name: stringify!($l),
            args: sa! [
                arg!(Int width = 1),
            ],
            initialise: Level::<$l>::initialise,
            tick: Level::<$l>::tick,
            get_meta: Level::<$l>::get_meta,
        }
    };
}

pub(super) use level;
