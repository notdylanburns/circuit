use circuit_extlib::rust::*;
use circuit_extlib::*;

mod probe;

pub(super) fn init(build_consts: &BuildConsts) -> Library {
    library! {
        name: "debug",
        circs: sa![],
        enums: sa![],
        libraries: sa![
            probe::init(build_consts),
        ],
    }
}
