use std::fmt::Display;
use std::ops::Deref;

use crate::ref_util::{Ref, MakeRef};
use super::{StateArray, TruthTable};
use super::state::State;
use super::pin::{Pin, PinConnection, SymbolicPin, PinDirection};
use super::truth_table::truth_table;

pub(super) struct SymbolicPinConnection {
    start_index: usize,
    end_index: usize,
    pin: usize,
}

pub(super) struct SymbolicConnection {
    width: usize,
    circ_pins: Vec<(usize, SymbolicPinConnection)>,     // (circuit index, pin index)
    sym_pins: Vec<SymbolicPinConnection>,               // Pin indicies
}

impl SymbolicConnection {
    pub fn new(width: usize) -> Self {
        Self {
            width,
            circ_pins: vec![],
            sym_pins: vec![],
        }
    }

    pub fn add_circ_pin_single(mut self, circ_num: usize, pin_num: usize) -> Self {
        self.add_circ_pin(circ_num, pin_num, 0, 0)
    }

    pub fn add_sym_pin_single(mut self, pin_num: usize) -> Self {
        self.add_sym_pin(pin_num, 0, 0)
    }

    pub fn add_circ_pin(mut self, circ_num: usize, pin_num: usize, start_index: usize, end_index: usize) -> Self {
        assert!(end_index >= start_index && (end_index - start_index + 1) == self.width);

        self.circ_pins.push((circ_num, SymbolicPinConnection { start_index, end_index, pin: pin_num }));
        self
    }

    pub fn add_sym_pin(mut self, pin_num: usize, start_index: usize, end_index: usize) -> Self {
        assert!(end_index >= start_index && (end_index - start_index + 1) == self.width);

        self.sym_pins.push(SymbolicPinConnection { start_index, end_index, pin: pin_num });
        self
    }
}

pub(super) struct Connection {
    width: usize,
    pins: Vec<Ref<PinConnection>>,
    value: StateArray,
}

impl Connection {
    pub fn new(pins: Vec<Ref<PinConnection>>) -> Self {
        let mut width = None;
        for pin in pins.iter() {
            let pin_width = pin.with(|p| p.width());
            if let Some(w) = width {
                if pin_width != w {
                    panic!("Invalid pin width in connection: {pin_width} != {w}")
                }
            } else {
                width = Some(pin_width);
            };
        };

        let width = width.expect("No pin connections specified!");
        let states: Vec<_> = pins.iter()
            .map(|ref_p| 
                ref_p.with(|p| p.read().clone())
            )
            .collect();

        let value = StateArray::merge_all(&states);

        Self {
            pins,
            width,
            value,
        }

    }

    pub fn update(&mut self) {
        let mut states = self.pins
            .iter()
            .map(|pin| pin.with(|p| p.read()))
            .collect::<Vec<_>>();

        self.value = states
            .iter()
            .fold(StateArray::new(self.width), |state1, state2| &state1 | state2);

        self.pins
            .iter_mut()
            .zip(states)
            .filter_map(|(pin, state)|
                if state == self.value {
                    None
                } else {
                    Some(pin)
                }
            )
            .for_each(|pin|
                pin.with_mut(|mut p| {
                    p.write(&self.value.as_vec())
                })
            )
    }
}

pub(super) struct CircuitBuilder<'a> {
    circuit: CircuitTemplate<'a>,
}

impl<'a> CircuitBuilder<'a> {
    pub fn new() -> Self {
        Self {
            circuit: CircuitTemplate {
                truth_table: None,
                pins: vec![],
                circuits: vec![],
                connections: vec![],
            }
        }
    }

    pub fn from_truth_table(truth_table: TruthTable) -> CircuitTemplate<'a> {
        let mut builder = Self::new();

        let (input_count, output_count) = truth_table.input_output_count();

        for _ in 0..input_count {
            let pin_id = builder.add_pin(Pin::new(1, PinDirection::Input));
            builder.add_connection(SymbolicConnection::new(1).add_sym_pin(pin_id, 0, 0));
        };

        for _ in 0..output_count {
            let pin_id = builder.add_pin(Pin::new(1, PinDirection::Output));
            builder.add_connection(SymbolicConnection::new(1).add_sym_pin(pin_id, 0, 0))
        };

        let mut circuit = builder.build();
        circuit.truth_table = Some(truth_table);

        circuit
    }

    pub fn create_pin(&mut self, width: usize, direction: PinDirection) -> usize {
        let pin_id = self.circuit.pins.len();
        self.circuit.pins.push(
            Pin::new(width, direction)
        );
        pin_id
    }

    pub fn add_pin(&mut self, pin: Pin) -> usize {
        let pin_id = self.circuit.pins.len();
        self.circuit.pins.push(pin);
        pin_id
    }

    pub fn add_circuit(&mut self, circuit: &'a CircuitTemplate) -> usize {
        let circuit_id = self.circuit.circuits.len();
        self.circuit.circuits.push(circuit);
        circuit_id
    }

    pub fn add_connection(&mut self, connection: SymbolicConnection) {
        self.circuit.connections.push(connection);
    }

    pub fn build(self) -> CircuitTemplate<'a> {
        self.circuit
    }
}

pub(super) struct CircuitTemplate<'a> {
    truth_table: Option<TruthTable>,
    pins: Vec<Pin>,
    circuits: Vec<&'a CircuitTemplate<'a>>,
    connections: Vec<SymbolicConnection>,
}

impl<'a> CircuitTemplate<'a> {
    /// When generating a truth table based circuit, also generate connections for each pin to be used internally
    pub fn new(pins: Vec<Pin>, circuits: Vec<&'a CircuitTemplate>, connections: Vec<SymbolicConnection>) -> Self {
        let mut can_create_tt = true;
        let mut input_count = 0;
        let mut output_count = 0;

        pins.iter()
            .for_each(|p| {
                match p.direction() {
                    PinDirection::Any => can_create_tt = false,
                    PinDirection::Input => input_count += 1,
                    PinDirection::Output => output_count += 1,
                }
            });

        let mut circuit_template = Self {
            truth_table: None,
            pins,
            circuits,
            connections,
        };

        if can_create_tt {
            circuit_template.create_truth_table(input_count, output_count);
        }

        circuit_template
    }

    pub fn take_truth_table(self) -> Option<TruthTable> {
        self.truth_table
    }

    fn create_truth_table(&mut self, input_count: usize, output_count: usize) {
        // TODO: implement
    }

    pub fn instantiate(&self) -> Circuit {
        let pins: Vec<Ref<Pin>> = self.pins
            .iter()
            .map(|p| p.clone().into_ref())
            .collect();

        let circuits: Vec<Circuit> = self.circuits
            .iter()
            .map(|c| c.instantiate())
            .collect();

        let connections: Vec<Connection> = self.connections.iter()
            .map(|c| {
                let mut conns: Vec<Ref<PinConnection>> = c.circ_pins
                    .iter()
                    .map(|(circ_idx, conn)|
                        PinConnection::new(
                            circuits[*circ_idx].pins[conn.pin].clone(),
                            conn.start_index,
                            conn.end_index,
                        ).into_ref()
                    )
                    .collect();

                conns.extend(c.sym_pins
                    .iter()
                    .map(|conn|
                        PinConnection::new(
                            pins[conn.pin].clone(),
                            conn.start_index,
                            conn.end_index,
                        ).into_ref()
                    )
                );

                Connection::new(conns)
            })
            .collect();

        Circuit {
            truth_table: self.truth_table.clone(),
            pins,
            circuits,
            connections,
            dirty: false,
        }
    }
}

pub(super) struct Circuit {
    truth_table: Option<TruthTable>,
    pins: Vec<Ref<Pin>>,
    circuits: Vec<Circuit>,
    connections: Vec<Connection>,
    dirty: bool,
}

impl Circuit {
    pub fn tick(&mut self) {
        if self.truth_table.is_some() {
            self.process_truth_table();
        } else {
            self.process_update();
        }

        self.dirty = false;
    }

    // pub fn set_pin(&mut self, pin_num: usize, state: &[State]) {
    //     self.pins[pin_num].with_mut(|mut p| p.set_state(state));
    //     self.dirty = true;
    // }

    // pub fn get_pin(&mut self, pin_num: usize) -> Vec<State> {
    //     self.pins[pin_num].with(|p| p.get_state().to_owned())
    // }

    fn process_truth_table(&mut self) {
        let tt = self.truth_table.as_mut().unwrap();
        let (input_count, output_count) = tt.input_output_count();
        let mut input_value = StateArray::new(input_count);
        let mut output_value = &StateArray::new(0);
        self.connections.iter_mut()
            .enumerate()
            .for_each(|(idx, connection)| {
                let mut pin = &mut connection.pins[0];
                // If index indicates that we are on an input pin
                if idx < input_count {
                    // For each bit of the pin state, shift the value into the input
                    for v in pin.with(|p| p.read()).iter() {
                        input_value.set(idx, &v)
                    }
                    return
                }

                let idx = idx - input_count;

                // If we have just finished the input pins, calculate the output value
                if idx == 0 {
                    output_value = tt.get(&input_value);
                }

                pin.with_mut(|mut p| p.write(&[output_value.get(idx)]));
            });
    }

    fn process_update(&mut self) {
        // Read inputs
        self.update_connections();

        // Generate outputs
        self.update_circuits();

        // Write outputs
        self.update_connections();
    }

    fn update_connections(&mut self) {
        self.connections
            .iter_mut()
            .for_each(|mut conn| conn.update());
    }

    fn update_circuits(&mut self) {
        self.circuits
            .iter_mut()
            .for_each(|mut circ| circ.tick());
    }

    pub fn connect(&self, pin_index: usize, start_index: usize, end_index: usize) -> PinConnection {
        PinConnection::new(self.pins[pin_index].clone(), start_index, end_index)
    }
}


#[test]
fn test() {
    // let mut and = Circuit {
    //     truth_table: Some(truth_table!(
            // [TruthTableEntry; 4]
            // 0 0 | 0,
            // 0 1 | 0,
            // 1 0 | 0,
            // 1 1 | 1
    //     )),
    //     pin_count: 2,
    //     input_count: 1,
    //     output_count: 1,
    //     pins: vec![
    //         Pin::new(2).as_ref(),
    //         Pin::new(1).as_ref(),
    //     ],
    //     connections: vec![],
    //     circuits: vec![],
    // };

    let and_template = CircuitBuilder::from_truth_table(truth_table!(
        [ 2 | 1 ]
        Z Z | Z,
        Z L | Z,
        Z H | Z,
        L Z | Z,
        L L | L,
        L H | L,
        H Z | Z,
        H L | L,
        H H | H,
    ));

    let mut and = and_template.instantiate();
    let mut conn_a = PinConnection::new(and.pins[0].clone(), 0, 0);
    let mut conn_b = PinConnection::new(and.pins[1].clone(), 0, 0);
    let conn_out = PinConnection::new(and.pins[2].clone(), 0, 0);
    
    conn_a.write(&[State::H]);
    conn_b.write(&[State::H]);
    and.tick();
    println!("{:?}", conn_out.read());

    conn_b.write(&[State::L]);
    and.tick();
    println!("{:?}", conn_out.read());

    conn_b.write(&[State::Z]);
    and.tick();
    println!("{:?}", conn_out.read());

    conn_b.write(&[State::Z]);
    and.tick();
    println!("{:?}", conn_out.read());
}

#[test]
fn build_myand() {
    let and_template = CircuitBuilder::from_truth_table(truth_table!(
        [ 2 | 1 ]
        L L | L,
        L H | L,
        H L | L,
        H H | H,
    ));

    let mut myand_builder = CircuitBuilder::new();
    let input_a_index = myand_builder.create_pin(1, PinDirection::Input);
    let input_b_index = myand_builder.create_pin(1, PinDirection::Input);
    let output_index = myand_builder.create_pin(1, PinDirection::Output);

    let and_circuit_index = myand_builder.add_circuit(&and_template);

    myand_builder.add_connection(
        SymbolicConnection::new(1)
            .add_circ_pin_single(and_circuit_index, 0)
            .add_sym_pin_single(input_a_index)
    );

    myand_builder.add_connection(
        SymbolicConnection::new(1)
            .add_circ_pin_single(and_circuit_index, 1)
            .add_sym_pin_single(input_b_index)
    );

    myand_builder.add_connection(
        SymbolicConnection::new(1)
            .add_circ_pin_single(and_circuit_index, 2)
            .add_sym_pin_single(output_index)
    );

    let myand_template = myand_builder.build();
    let mut myand = myand_template.instantiate();

    let mut conn_a = myand.connect(input_a_index, 0, 0);
    let mut conn_b = myand.connect(input_b_index, 0, 0);
    let conn_out = myand.connect(output_index, 0, 0);

    conn_a.write(&[State::L]);
    conn_b.write(&[State::L]);
    myand.tick();
    println!("{:?}", conn_out.read());

    conn_b.write(&[State::H]);
    myand.tick();
    println!("{:?}", conn_out.read());

    conn_a.write(&[State::H]);
    myand.tick();
    println!("{:?}", conn_out.read());

}

#[test]
fn tristate_test() {
    let mut ts_buffer = CircuitBuilder::from_truth_table(truth_table!(
        [ 2 | 1 ]  // 1: Input, 2: TriState trigger
        Z L | Z,
        L L | Z,
        H L | Z,
        Z H | Z,
        L H | L,
        H H | H,
    )).instantiate();

    let mut conn_input = ts_buffer.connect(0, 0, 0);
    let mut conn_switch = ts_buffer.connect(1, 0, 0);
    let conn_out = ts_buffer.connect(2, 0, 0);

    conn_input.write(&[State::H]);
    conn_switch.write(&[State::L]);
    ts_buffer.tick();

    println!("{:?}", conn_out.read());

    conn_switch.write(&[State::H]);
    ts_buffer.tick();

    println!("{:?}", conn_out.read());

    conn_switch.write(&[State::Z]);
    ts_buffer.tick();

    println!("{:?}", conn_out.read());
}