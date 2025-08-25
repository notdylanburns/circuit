mod bus;
mod debug;
mod emu;
mod logic;

use circuit_extlib::rust::*;
use circuit_extlib::*;

#[no_mangle]
pub extern "C" fn init(argc: usize, argv: *const BuildConst) -> InitResult {
    let build_consts = get_build_consts(argc, argv);

    let library = library! {
        name: "core",
        circs: sa![],
        enums: sa![],
        libraries: sa![
            bus::init(&build_consts),
            debug::init(&build_consts),
            logic::init(&build_consts),
        ],
    };

    InitResult {
        library,
        error: std::ptr::null(),
    }
}
