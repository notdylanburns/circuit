use circuit_extlib::rust::*;
use circuit_extlib::*;

struct ArgParser {
    width: usize,
}

impl ArgParser {
    fn parse(args: Args) -> Result<Self, &'static str> {
        use ExtractedConst::*;

        let [Int(width)] = args else {
            unreachable!("invalid arg types: {args:?}");
        };

        if *width <= 0 {
            return Err("width must be at least 1");
        }

        Ok(Self {
            width: *width as usize,
        })
    }
}

trait Gate {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State>;
}

struct BasicGate<G: Gate> {
    width: usize,
    _marker: std::marker::PhantomData<G>,
}

impl<G: Gate> Circ for BasicGate<G> {
    fn new(args: Args) -> Self {
        let args = ArgParser::parse(args).unwrap();

        Self {
            width: args.width,
            _marker: std::marker::PhantomData,
        }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        let inputs = state.read_inputs(self.width * 2);

        let a = &inputs[..self.width];
        let b = &inputs[self.width..];

        let outputs = G::evaluate(a, b);
        state.write_outputs(&outputs);
    }

    fn get_meta_internal(args: Args) -> Result<CircMeta, &'static str> {
        let args = ArgParser::parse(args)?;

        Ok(circ_meta! {
            mem_size: std::mem::size_of::<Self>(),
            pins: sa![
                pin!(Input a[args.width]),
                pin!(Input b[args.width]),
                pin!(Output q[args.width]),
            ],
            tick_behaviour: TickBehaviour::Once,
        })
    }
}

struct And;
impl Gate for And {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::L, _) | (_, State::L) => State::L,
                (State::H, State::H) => State::H,
            })
            .collect()
    }
}

struct Nand;
impl Gate for Nand {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::L, _) | (_, State::L) => State::H,
                _ => State::L,
            })
            .collect()
    }
}

struct Nor;
impl Gate for Nor {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::H, _) | (_, State::H) => State::L,
                _ => State::H,
            })
            .collect()
    }
}

struct Or;
impl Gate for Or {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::H, _) | (_, State::H) => State::H,
                _ => State::L,
            })
            .collect()
    }
}

struct Xnor;
impl Gate for Xnor {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::H, State::H) | (State::L, State::L) => State::H,
                _ => State::L,
            })
            .collect()
    }
}

struct Xor;
impl Gate for Xor {
    fn evaluate(a: &[State], b: &[State]) -> Vec<State> {
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| match (a, b) {
                (State::E, _) | (_, State::E) => State::E,
                (State::Z, _) | (_, State::Z) => State::Z,
                (State::H, State::H) | (State::L, State::L) => State::L,
                _ => State::H,
            })
            .collect()
    }
}

macro_rules! gate {
    ($n:ident) => {
        circ! {
            name: stringify!($n),
            args: sa! [
                arg!(Int width = 1),
            ],
            initialise: BasicGate::<$n>::initialise,
            tick: BasicGate::<$n>::tick,
            get_meta: BasicGate::<$n>::get_meta,
        }
    };
}

pub(super) fn init(_: &BuildConsts) -> Library {
    library! {
        name: "logic",
        circs: sa![
            gate!(And),
            gate!(Nand),
            gate!(Nor),
            gate!(Or),
            gate!(Xnor),
            gate!(Xor),
        ],
        enums: sa![],
        libraries: sa![],
    }
}
