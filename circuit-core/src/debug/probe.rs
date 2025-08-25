use circuit_extlib::rust::*;
use circuit_extlib::*;

struct ArgParser {
    width: usize,
    tick_behaviour: TickBehaviour,
}

impl ArgParser {
    fn parse(args: Args) -> Self {
        use ExtractedConst::*;

        let [Int(width)] = args else {
            unreachable!("invalid arg types")
        };

        Self {
            width: *width as usize,
            tick_behaviour: TickBehaviour::OnChange,
        }
    }
}

pub struct Probe {
    width: usize,
}

impl Circ for Probe {
    fn new(args: Args) -> Self {
        let args = ArgParser::parse(args);

        Self { width: args.width }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        let inputs = state.read_inputs(self.width);

        println!("probe: [tick={}] {:?} ", state.rt.ticks, inputs);
    }

    fn get_meta_internal(args: Args) -> Result<CircMeta, &'static str> {
        let args = ArgParser::parse(args);

        if args.width == 0 {
            Err("width cannot be 0")
        } else {
            Ok(circ_meta! {
                mem_size: std::mem::size_of::<Self>(),
                pins: sa! [
                    pin!(Input a[args.width]),
                ],
                tick_behaviour: args.tick_behaviour,
            })
        }
    }
}

pub(super) fn init(_: &BuildConsts) -> Library {
    library! {
        name: "probe",
        circs: sa! [
            circ! {
                name: "Probe",
                args: sa! [
                    arg!(Int width = 1),
                ],
                initialise: Probe::initialise,
                tick: Probe::tick,
                get_meta: Probe::get_meta,
            }
        ],
        enums: sa![],
        libraries: sa![],
    }
}
