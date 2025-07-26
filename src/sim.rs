mod state;
mod pin;
mod circuit;
#[macro_use]
mod truth_table;

mod core;

type StateArray = state::StateArray<usize>;
type TruthTableEntry = StateArray;
type TruthTable = truth_table::TruthTable;

#[test]
fn test_circ() {
    use core::CoreCircuit;
    use state::State;
    use pin::PinDirection;
    use circuit::{Circuit, CircuitBuilder, CircuitTemplate, Connection, SymbolicConnection};
    use crate::ref_util::MakeRef;

    let nor_template = CoreCircuit::Nor(2, State::Z).build();
    let mut sr_builder = CircuitBuilder::new();
    
    let nor1_id = sr_builder.add_circuit(&nor_template);
    let nor2_id = sr_builder.add_circuit(&nor_template);

    let s_id = sr_builder.create_pin(1, PinDirection::Input);
    let r_id = sr_builder.create_pin(1, PinDirection::Input);
    let q_id = sr_builder.create_pin(1, PinDirection::Output);

    sr_builder.add_connection(
        SymbolicConnection::new(1)
            .add_sym_pin_single(s_id)
            .add_circ_pin_single(nor1_id, 0)
    );

    sr_builder.add_connection(
        SymbolicConnection::new(1)
            .add_sym_pin_single(r_id)
            .add_circ_pin_single(nor2_id, 0)
    );

    sr_builder.add_connection(
        SymbolicConnection::new(1)
            .add_circ_pin_single(nor1_id, 1)
            .add_circ_pin_single(nor2_id, 2)
    );

    sr_builder.add_connection(
        SymbolicConnection::new(1)
            .add_sym_pin_single(r_id)
            .add_circ_pin_single(nor1_id, 2)
            .add_circ_pin_single(nor2_id, 1)
    );

    let mut sr = sr_builder.build().instantiate();

    let mut conn_s = sr.connect(s_id, 0, 0);
    let mut conn_r = sr.connect(r_id, 0, 0);
    let conn_q = sr.connect(q_id, 0, 0);

    conn_s.write(&[State::H]);
    sr.tick();
    println!("{:?}", conn_q.read());

    conn_s.write(&[State::L]);
    conn_r.write(&[State::H]);
    sr.tick();
    println!("{:?}", conn_q.read());
}