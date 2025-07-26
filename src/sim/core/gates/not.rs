use super::super::{CircuitBuilder, CircuitTemplate, State, state_array, TruthTable};

pub fn build_not(pull: State) -> CircuitTemplate<'static> {
    let mut tt = TruthTable::new(1, 1);
    tt.set(&state_array![State::L], state_array![State::H]);
    tt.set(&state_array![State::H], state_array![State::L]);
    tt.set(&state_array![State::E], state_array![State::E]);

    let result = match pull {
        State::Z => State::Z,
        State::L => State::H,
        State::H => State::L,
        State::E => State::E,
    };

    tt.set(&state_array![State::Z], state_array![result]);

    CircuitBuilder::from_truth_table(tt)
}