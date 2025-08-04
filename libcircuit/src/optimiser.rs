mod ir;
mod truth_table;

use crate::analyser::{Circ, CircId};
use crate::codegen::targets::ir::IrBlocks;
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

    pub fn optimise_ir(blocks: IrBlocks) -> IrBlocks {
        IrOptimiser::optimise(blocks)
    }

    // fn get_bit_counts(main: CircId, circs: &[Circ]) -> Vec<usize> {
    //     let mut bitcount = vec![None; circs.len()];

    //     bitcount[main] = Some(Self::get_circ_bit_count(&mut bitcount, circs, &circs[main]));

    //     bitcount
    //         .into_iter()
    //         .map(|bc| bc.unwrap_or_else(|| unreachable!()))
    //         .collect()
    // }

    // fn get_circ_bit_count(bitcount: &mut [Option<usize>], circs: &[Circ], circ: &Circ) -> usize {
    //     let mut count = 0;
    //     for &dep in circ.dependencies.iter() {
    //         if let Some(bs) = bitcount[dep] {
    //             count += bs;
    //         } else {
    //             let bs = Self::get_circ_bit_count(bitcount, &circs, &circs[dep]);
    //             bitcount[dep] = Some(bs);
    //             count += bs;
    //         }
    //     }

    //     count
    //         + circ
    //             .pins
    //             .values()
    //             .map(|&Pin { width, .. }| width)
    //             .sum::<usize>()
    // }
}
