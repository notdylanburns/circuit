mod ir;
mod truth_table;

use crate::analyser::{Circ, CircId};
use crate::codegen::targets::ir::blocks::Sections;
use crate::codegen::targets::ir::WordTrait;
use ir::IrOptimiser;
use truth_table::TruthTableOptimiser;
pub use truth_table::{BitEndpoint, TruthTable};

#[derive(Debug)]
pub enum OptimiserUnit {
    Circ(Circ),
    TruthTable(TruthTable),
}

#[derive(Debug)]
pub struct Optimiser {
    main: CircId,
    circs: Vec<OptimiserUnit>,
}

impl Optimiser {
    pub fn optimise_circs(circs: Vec<Circ>, main: CircId) -> Vec<OptimiserUnit> {
        let mut units = circs
            .into_iter()
            .map(OptimiserUnit::Circ)
            .collect::<Vec<_>>();

        TruthTableOptimiser::optimise(&mut units[..], main);

        units
    }

    pub fn optimise_ir<Word: WordTrait>(blocks: Sections<Word>) -> Sections<Word> {
        IrOptimiser::optimise(blocks)
    }
}
