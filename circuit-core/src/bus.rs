use circuit_extlib::rust::*;
use circuit_extlib::*;

mod constant;
mod level;

use level::{level, Level, E, H, L, Z};

pub(super) fn init(_: &BuildConsts) -> Library {
    library! {
        name: "bus",
        circs: sa! [
            level!(Z),
            level!(L),
            level!(H),
            level!(E),
            circ! {
                name: "Constant",
                args: sa! [
                    arg!(Int value),
                    arg!(Int width = 1),
                ],
                initialise: constant::Constant::initialise,
                tick: constant::Constant::tick,
                get_meta: constant::Constant::get_meta,
            }
        ],
        enums: sa![],
        libraries: sa![],
    }
}
