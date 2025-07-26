use super::super::{CircuitBuilder, CircuitTemplate, State, StateArray, TruthTable};

pub fn build_and(input_count: usize, pull: State) -> CircuitTemplate<'static> {
    let mut tt = TruthTable::new(input_count, 1);

    StateArray::permutations(input_count)
        .for_each(|sa| {
            let mut state_count = [0usize; State::STATE_COUNT];

            sa.iter()
                .map(|s| 
                    if matches!(s, State::Z) {
                        pull
                    } else {
                        s
                    }
                )
                .for_each(|s| state_count[s.num::<usize>()] += 1);

            if state_count[State::E.num::<usize>()] > 0 {
                tt.set(&sa, StateArray::from(&[State::E]))
            } else if state_count[State::Z.num::<usize>()] > 0 {
                tt.set(&sa, StateArray::from(&[State::Z]))
            } else if state_count[State::L.num::<usize>()] > 0 {
                tt.set(&sa, StateArray::from(&[State::L]))
            } else {
                tt.set(&sa, StateArray::from(&[State::H]))
            }
        });

    CircuitBuilder::from_truth_table(tt)
}

pub fn build_nand(input_count: usize, pull: State) -> CircuitTemplate<'static> {
    let mut tt = build_and(input_count, pull).take_truth_table().unwrap();
    tt.negate_outputs();

    CircuitBuilder::from_truth_table(tt)
}