use core::mem::size_of;
use std::slice::SliceIndex;

use super::{StateArray, TruthTableEntry};
use super::state::State;

// #[derive(Clone)]
// pub(super) struct TruthTable<T: Copy + Default> {
//     entries: usize,
//     inputs: usize,
//     outputs: usize,
//     values: Vec<T>,
// }

// impl<T: Copy + Default> TruthTable<T> {
//     pub fn new(entries: usize, inputs: usize, outputs: usize) -> Self {
//         Self {
//             entries,
//             inputs,
//             outputs,
//             values: vec![T::default(); entries],
//         }
//     }

//     pub fn input_output_count(&self) -> (usize, usize) {
//         (self.inputs, self.outputs)
//     }

//     pub fn bounds_check(&self, inputs: usize) {
//         if inputs > self.entries {
//             panic!("Index out of bounds: {} > {}", inputs, self.entries);
//         };
//     }

//     pub fn set_unchecked(&mut self, inputs: usize, outputs: T) -> &mut Self {
//         self.values[inputs] = outputs;
//         self
//     }

//     pub fn set(&mut self, inputs: usize, outputs: T) -> &mut Self {
//         self.bounds_check(inputs);
//         self.set_unchecked(inputs, outputs)
//     }

//     pub fn get_unchecked(&self, inputs: usize) -> T {
//         self.values[inputs]
//     }

//     pub fn get(&self, inputs: usize) -> T {
//         self.bounds_check(inputs);
//         self.get_unchecked(inputs)
//     }
// }

#[derive(Clone)]
pub struct TruthTable {
    inputs: usize,
    outputs: usize,
    values: Vec<TruthTableEntry>,
}

impl TruthTable {
    const MAX_INPUTS: usize = TruthTableEntry::COUNT;

    pub fn new(inputs: usize, outputs: usize) -> Self {
        assert!(inputs <= Self::MAX_INPUTS);

        Self {
            inputs,
            outputs,
            values: vec![StateArray::error(outputs); State::STATE_COUNT.pow(inputs as u32)],
        }
    }

    pub const fn input_output_count(&self) -> (usize, usize) {
        (self.inputs, self.outputs)
    }

    fn bounds_check(inputs: &TruthTableEntry) {
        assert!(inputs.get_value() < Self::MAX_INPUTS);
    }

    pub fn set_unchecked(&mut self, inputs: &TruthTableEntry, outputs: TruthTableEntry) {
        self.values[inputs.get_value()] = outputs;
    }

    pub fn set(&mut self, inputs: &TruthTableEntry, outputs: TruthTableEntry) {
        Self::bounds_check(inputs);
        self.set_unchecked(inputs, outputs)
    }

    pub fn set_all(&mut self, output: TruthTableEntry) {
        for value in self.values.iter_mut() {
            *value = output.clone();
        }
    }

    pub fn get_unchecked(&self, inputs: &TruthTableEntry) -> &TruthTableEntry {
        &self.values[inputs.get_value()]
    }

    pub fn get(&self, inputs: &TruthTableEntry) -> &TruthTableEntry {
        Self::bounds_check(inputs);
        self.get_unchecked(inputs)
    }

    pub fn negate_outputs(&mut self) {
        self.values
            .iter_mut()
            .for_each(|v| *v = v.negate());
    }
}

macro_rules! truth_table {
    (
        [$I:literal | $O:literal] 
        $(
            $($i:ident)+|$($o:ident)+
        ),+
        $(,)?
    ) => {
        {
            let mut tt = $crate::sim::truth_table::TruthTable::new($I, $O);
            $(
                let mut input_val = $crate::sim::state::StateArray::new($I);
                let mut output_val = $crate::sim::state::StateArray::new($O);
                let mut input_index = 0;
                let mut output_index = 0;

                $(
                    input_val.set(
                        input_index,
                        &$crate::sim::state::State::from(stringify!($i)),
                    );

                    input_index += 1;
                )+
                $(
                    output_val.set(
                        output_index,
                        &$crate::sim::state::State::from(stringify!($o)),
                    );

                    output_index += 1;
                )+

                tt.set(&input_val, output_val);
            )+

            tt
        }
    };
}

pub(super) use truth_table;
