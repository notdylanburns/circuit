use circuit_extlib::rust::*;
use circuit_extlib::*;

struct ArgParser {
    width: usize,
    value: isize,
}

impl ArgParser {
    fn parse(args: Args) -> Result<Self, &'static str> {
        use ExtractedConst::*;

        let &[Int(value), Int(width)] = args else {
            unreachable!("invalid arg types")
        };

        if width < 0 {
            return Err("width must be at least 0");
        }

        if width == 0 && value == 0 {
            Err("width cannot be 0 when value is 0")
        } else {
            Ok(Self {
                width: width as usize,
                value,
            })
        }
    }
}

pub(super) struct Constant {
    width: usize,
    value: isize,
}

impl Circ for Constant {
    fn new(args: Args) -> Self {
        let args = ArgParser::parse(args).unwrap();
        let width = if args.width == 0 {
            (isize::BITS - args.value.leading_zeros()) as usize
        } else {
            args.width
        };

        Self {
            width,
            value: args.value,
        }
    }

    fn tick_internal(&mut self, state: &mut SimState) {
        let mut outputs = vec![State::L; self.width];
        outputs.iter_mut().enumerate().for_each(|(i, s)| {
            if (self.value & (1 << i)) > 0 {
                *s = State::H
            }
        });

        state.write_outputs(&outputs);
    }

    fn get_meta_internal(args: Args) -> Result<CircMeta, &'static str> {
        let args = ArgParser::parse(args)?;

        dbg!(args.value, args.width);

        Ok(circ_meta! {
            mem_size: std::mem::size_of::<Self>(),
            pins: sa![
                pin!(Output q[args.width]),
            ],
            tick_behaviour: TickBehaviour::Once,
        })
    }
}
