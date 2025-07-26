mod gates;

use gates::*;

use super::circuit::{CircuitBuilder, CircuitTemplate};
use super::state::{State, StateArray, state_array};
use super::TruthTable;
use super::truth_table::truth_table;

macro_rules! core_circuit {
    (
        $(
            $n:ident $((
                $($f:ident: $t:ty),+
            ))? : $b:ident
        ),+
        $(,)?
    ) => {
        pub enum CoreCircuit {
            $(
                $n$(($($t),+))?
            ),+
        }

        impl CoreCircuit {
            pub fn build(self) -> CircuitTemplate<'static> {
                match self {
                    $(
                        Self::$n$(($($f),+))? => $b($($($f),+)?)
                    ),+
                }
            }
        }
    };
}

core_circuit! {
    And(n: usize, pull: State) : build_and,
    Nand(n: usize, pull: State) : build_nand,
    Or(n: usize, pull: State) : build_or,
    Nor(n: usize, pull: State) : build_nor,
    Xor(n: usize, odd: bool, pull: State) : build_xor,
    Xnor(n: usize, odd: bool, pull: State) : build_xnor,
    Not(pull: State): build_not,
}