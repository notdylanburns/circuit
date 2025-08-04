pub mod blocks;
pub mod targets;

use crate::{analyser::CircId, optimiser::OptimiserUnit};
use targets::ir::{IrBlocks, IrGen};

pub struct Codegen;

impl Codegen {
    pub fn emit_ir(units: &[OptimiserUnit], main: CircId) -> IrBlocks {
        let b = IrGen::new(units).emit(main);
        println!("{b}");

        b
    }
}
