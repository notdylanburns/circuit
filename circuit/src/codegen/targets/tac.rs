use crate::{
    analyser::Circ,
    codegen::blocks::Blocks,
    optimiser::{OptimiserUnit, TruthTable},
};

#[derive(Debug, Clone, Copy)]
pub enum Code {
    LoadInput {
        into_id: usize,
        bit_start: usize,
        bit_end: usize,
    },
    Jump {
        target_block: usize,
    },
    InitialiseFromMem {
        id: usize,
        target_block: usize,
        address: usize,
    },
    InitialiseFromConst {
        id: usize,
        value: usize,
    },
}

pub struct TacGen<'a> {
    units: &'a [OptimiserUnit],
    blocks: Blocks<Code, usize>,
    next_id: usize,
}

impl<'a> TacGen<'a> {
    pub fn new(units: &'a [OptimiserUnit]) -> Self {
        Self {
            units,
            blocks: Blocks::default(),
            next_id: 0,
        }
    }

    pub fn generate(mut self) -> Blocks<Code, usize> {
        for unit in self.units {
            match unit {
                OptimiserUnit::Circ(circ) => self.emit_for_circ(circ),
                OptimiserUnit::TruthTable(tt) => self.emit_for_truth_table(tt),
            }
        }

        self.blocks
    }

    fn get_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn emit_for_circ(&mut self, circ: &Circ) {}

    fn emit_for_truth_table(&mut self, truth_table: &TruthTable) {
        let mut tt_data = self.blocks.new_data();
        let row_size = 2 * truth_table.outputs.len() / (usize::BITS as usize);
        tt_data.push(row_size);

        // for row in truth_table.iter_rows() {
        //     row.value()
        // }

        // tt_data.push();
    }
}
