use super::super::{CircuitBuilder, CircuitTemplate, State, StateArray, TruthTable};

pub fn build_or(input_count: usize, pull: State) -> CircuitTemplate<'static> {
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
            } else if state_count[State::H.num::<usize>()] > 0 {
                tt.set(&sa, StateArray::from(&[State::H]))
            } else {
                tt.set(&sa, StateArray::from(&[State::L]))
            }
        });

    CircuitBuilder::from_truth_table(tt)
}

pub fn build_nor(input_count: usize, pull: State) -> CircuitTemplate<'static> {
    let mut tt = build_or(input_count, pull).take_truth_table().unwrap();
    tt.negate_outputs();

    CircuitBuilder::from_truth_table(tt)
}

#[test]
fn test_or() {
    let mut or = build_or(2, State::Z).instantiate();
    let mut input_a = or.connect(0, 0, 0);
    let mut input_b = or.connect(1, 0, 0);
    let out = or.connect(2, 0, 0);

    input_a.write(&[State::L]);
    input_b.write(&[State::L]);
    or.tick();
    assert!(matches!(out.read().get(0), State::L));

    input_a.write(&[State::H]);
    input_b.write(&[State::L]);
    or.tick();
    assert!(matches!(out.read().get(0), State::H));

    input_a.write(&[State::L]);
    input_b.write(&[State::H]);
    or.tick();
    assert!(matches!(out.read().get(0), State::H));

    input_a.write(&[State::H]);
    input_b.write(&[State::H]);
    or.tick();
    assert!(matches!(out.read().get(0), State::H));
}