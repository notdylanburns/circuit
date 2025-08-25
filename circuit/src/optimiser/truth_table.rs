use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::{
    analyser::{
        Circ, CircId, ConnectionEndpoint, ConnectionRange, ConnectionType, ModuleCirc, PinId,
    },
    ast::PinDirection,
    util::Interner,
};

use super::OptimiserUnit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Z,
    L,
    H,
    E,
}

impl Value {
    fn value(&self) -> u8 {
        match self {
            Self::Z => 0,
            Self::L => 1,
            Self::H => 2,
            Self::E => 3,
        }
    }

    fn combine(&self, other: &Self) -> Self {
        Self::from(self.value() | other.value())
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Z,
            1 => Self::L,
            2 => Self::H,
            3 => Self::E,
            _ => unreachable!("invalid value supplied to Value::from {value}"),
        }
    }
}

#[derive(Debug)]
pub struct BitString(Box<[Value]>);

impl BitString {
    pub fn value(&self) -> u128 {
        self.0
            .iter()
            .rfold(0, |acc, x| (acc << 2) + x.value() as u128)
    }

    fn from_value(v: u128, count: usize) -> Self {
        let mut result = vec![Value::Z; count];
        for i in 0..count {
            let shift = 2 * i;
            let value = (((0b11 << shift) & v) >> shift) as u8;
            result[i] = Value::from(value);
        }

        Self(Box::from(&result[..]))
    }
}

impl FromIterator<Value> for BitString {
    fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
        Self::from(&iter.into_iter().collect::<Vec<_>>()[..])
    }
}

impl From<&[Value]> for BitString {
    fn from(value: &[Value]) -> Self {
        Self(Box::from(value))
    }
}

impl std::ops::Deref for BitString {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

#[derive(Debug)]
pub struct TreeTable {
    inputs: HashMap<BitEndpoint, usize>,
    rows: Vec<Value>,
}

impl TreeTable {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn empty(inputs: HashMap<BitEndpoint, usize>) -> Self {
        let row_count = 4u32.pow(inputs.len() as u32) as usize;

        Self {
            inputs,
            rows: vec![Value::Z; row_count],
        }
    }

    fn identity(input: BitEndpoint) -> Self {
        Self {
            inputs: HashMap::from([(input, 0)]),
            rows: vec![Value::Z, Value::L, Value::H, Value::E],
        }
    }

    fn get_input_row(&self, inputs: &HashMap<BitEndpoint, Value>) -> BitString {
        let mut row = vec![Value::Z; self.inputs.len()];
        for (endpoint, &index) in self.inputs.iter() {
            row[index] = inputs[endpoint];
        }

        BitString::from(&row[..])
    }

    fn query_map(&self, inputs: &HashMap<BitEndpoint, Value>) -> Value {
        self.query(self.get_input_row(inputs))
    }

    fn query(&self, input_row: BitString) -> Value {
        self.rows[input_row.value() as usize]
    }

    fn get_combined_value(&self, tts: &[Rc<Self>], row: u128) -> Value {
        let row_inputs = BitString::from_value(row, self.inputs.len());
        let row_inputs: HashMap<BitEndpoint, Value> = HashMap::from_iter(
            self.inputs
                .iter()
                .map(|(bit, index)| (*bit, row_inputs[*index])),
        );

        tts.iter()
            .map(|tt| tt.query_map(&row_inputs))
            .fold(Value::Z, |s, v| s.combine(&v))
    }

    fn combine(tts: &[Rc<Self>]) -> Self {
        let mut inputs = HashMap::new();
        for tt in tts.iter() {
            for input in tt.inputs.keys() {
                if !inputs.contains_key(input) {
                    inputs.insert(*input, inputs.len());
                }
            }
        }

        let mut result = Self::empty(inputs);
        for row in 0..result.row_count() {
            result.rows[row] = result.get_combined_value(tts, row as u128);
        }

        result
    }

    fn map_inputs(&self, mut inputs: HashMap<BitEndpoint, Option<Rc<Self>>>) -> Self {
        enum InputMapping {
            NotConnected,
            Direct(usize),
            TreeTable(Rc<TreeTable>),
        }

        let mut new_inputs = HashMap::new();
        let mut input_mapping: HashMap<usize, InputMapping> = HashMap::new();

        for (input, index) in self.inputs.iter() {
            let next_id = new_inputs.len();
            if let Some(tt) = inputs.remove(input) {
                if let Some(tt) = tt {
                    let tt = tt.clone();
                    for (i, input) in tt.inputs.keys().enumerate() {
                        new_inputs.entry(*input).or_insert(next_id + i);
                    }
                    input_mapping.insert(*index, InputMapping::TreeTable(tt));
                } else {
                    input_mapping.insert(*index, InputMapping::NotConnected);
                }
            } else {
                new_inputs.entry(*input).or_insert(next_id);
                input_mapping.insert(*index, InputMapping::Direct(next_id));
            }
        }

        let mut result = Self::empty(new_inputs);

        for row in 0..result.row_count() {
            let row_inputs = BitString::from_value(row as u128, result.inputs.len());
            let mut mapped_inputs = vec![Value::Z; self.inputs.len()];

            for (target_index, mapping) in input_mapping.iter() {
                let new_value = match mapping {
                    InputMapping::NotConnected => Value::Z,
                    InputMapping::Direct(source) => row_inputs[*source],
                    InputMapping::TreeTable(tt) => {
                        let tt_inputs = tt
                            .inputs
                            .iter()
                            .map(|(endpoint, _)| {
                                let input_index = result.inputs.get(endpoint).unwrap();
                                (*endpoint, row_inputs[*input_index])
                            })
                            .collect();

                        tt.query_map(&tt_inputs)
                    }
                };

                mapped_inputs[*target_index] = new_value;
            }

            let result_row = self.query(BitString::from(&mapped_inputs[..]));

            result.rows[row] = result_row;
        }

        result
    }
}

#[derive(Debug)]
pub struct TruthTable {
    pub inputs: HashMap<BitEndpoint, usize>,
    pub outputs: HashMap<BitEndpoint, usize>,
    rows: Vec<BitString>,
}

impl TruthTable {
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn get_named_input_row(
        inputs: &HashMap<BitEndpoint, usize>,
        row_n: u128,
    ) -> HashMap<BitEndpoint, Value> {
        let input = BitString::from_value(row_n, inputs.len());
        inputs
            .iter()
            .map(|(endpoint, index)| (*endpoint, input[*index]))
            .collect()
    }

    fn from_tree_tables(tts: HashMap<BitEndpoint, Rc<TreeTable>>) -> Self {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        for (output, tt) in tts.iter() {
            for input in tt.inputs.keys() {
                // if !inputs.contains_key(input) {
                //     inputs.insert(*input, inputs.len());
                // }
                inputs.push(*input);
            }

            // assert!(!outputs.contains_key(output));
            // outputs.insert(*output, outputs.len());
            outputs.push(*output);
        }

        inputs.sort();
        outputs.sort();

        inputs.dedup();
        // outputs should not contain duplicates

        let inputs: HashMap<_, _> = inputs
            .into_iter()
            .enumerate()
            .map(|(index, input)| (input, index))
            .collect();

        let outputs: HashMap<_, _> = outputs
            .into_iter()
            .enumerate()
            .map(|(index, output)| (output, index))
            .collect();

        let row_count = 4u32.pow(inputs.len() as u32) as usize;
        let mut rows = Vec::with_capacity(row_count);

        for row in 0..row_count {
            let row_input = Self::get_named_input_row(&inputs, row as u128);
            let mut row_output = vec![Value::Z; outputs.len()];
            for (output, tt) in tts.iter() {
                let index = outputs[output];
                row_output[index] = tt.query_map(&row_input);
            }

            rows.push(BitString::from(&row_output[..]));
        }

        Self {
            inputs,
            outputs,
            rows,
        }
    }

    fn get_tree_table(&self, pin: BitEndpoint) -> TreeTable {
        let BitEndpoint::Dependency {
            dependency_id,
            pin_id,
            bit,
        } = pin
        else {
            unreachable!();
        };

        let output_index = self.outputs.get(&BitEndpoint::Pin { pin_id, bit }).unwrap();

        TreeTable {
            inputs: self
                .inputs
                .iter()
                .map(|(endpoint, index)| {
                    let BitEndpoint::Pin { pin_id, bit } = endpoint else {
                        unreachable!()
                    };

                    (
                        BitEndpoint::Dependency {
                            dependency_id,
                            pin_id: *pin_id,
                            bit: *bit,
                        },
                        *index,
                    )
                })
                .collect(),
            rows: self.rows.iter().map(|r| r[*output_index]).collect(),
        }
    }

    pub fn iter_rows<'a>(&'a self) -> impl Iterator<Item = &'a BitString> {
        self.rows.iter()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord)]
pub enum BitEndpoint {
    Pin {
        pin_id: PinId,
        bit: usize,
    },
    Dependency {
        dependency_id: usize,
        pin_id: PinId,
        bit: usize,
    },
}

impl PartialOrd for BitEndpoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let BitEndpoint::Pin {
            pin_id: lp,
            bit: lb,
        } = self
        else {
            unreachable!();
        };

        let BitEndpoint::Pin {
            pin_id: rp,
            bit: rb,
        } = other
        else {
            unreachable!();
        };

        lp.partial_cmp(rp).map(|o| o.then(lb.cmp(rb)))
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct BitConnection {
    source: BitEndpointId,
    dest: BitEndpointId,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum ConnectionTree {
    NotConnected,
    Combine(Vec<Rc<Self>>),
    TruthTable(CircId, BitEndpoint),
    Source(BitEndpoint),
}

#[derive(Default)]
struct PinStats {
    input_count: usize,
    output_count: usize,
    output_pins: Vec<PinId>,
}

type BitConnectionId = usize;
type BitEndpointId = usize;

#[derive(Debug)]
struct TruthTableContext<'c> {
    circ: &'c ModuleCirc,
    endpoints: &'c Interner<BitEndpoint>,
    endpoint_inbound_connection_ids: &'c HashMap<BitEndpointId, Vec<BitConnectionId>>,
    bit_connections: &'c [BitConnection],
    tree_table_cache: HashMap<Rc<ConnectionTree>, Rc<TreeTable>>,
}

impl<'c> TruthTableContext<'c> {
    fn get_endpoint(&'c self, endpoint_id: BitEndpointId) -> &'c BitEndpoint {
        self.endpoints.get(endpoint_id).unwrap()
    }

    fn get_endpoint_inbound_connections(
        &'c self,
        endpoint_id: BitEndpointId,
    ) -> impl Iterator<Item = BitConnection> + 'c {
        self.endpoint_inbound_connection_ids[&endpoint_id]
            .iter()
            .map(|c| self.bit_connections[*c])
    }

    fn get_circ_dependency(&self, dependency_id: usize) -> CircId {
        self.circ.dependencies[dependency_id]
    }
}

pub(super) struct TruthTableOptimiser<'a> {
    circs: &'a mut [OptimiserUnit],
    attempted: HashSet<CircId>,
}

impl<'a> TruthTableOptimiser<'a> {
    pub(super) fn optimise(circs: &'a mut [OptimiserUnit], main: CircId) {
        Self {
            circs,
            attempted: HashSet::new(),
        }
        .generate_truth_tables(main)
    }

    fn get_pin_stats(circ: &ModuleCirc) -> PinStats {
        circ.pins
            .value_indices()
            .fold(PinStats::default(), |mut stats, (id, pin)| {
                if pin.direction == PinDirection::Input {
                    stats.input_count += pin.width;
                } else if pin.direction == PinDirection::Output {
                    stats.output_count += pin.width;
                    stats.output_pins.push(id);
                };
                stats
            })
    }

    fn generate_truth_tables(&mut self, main: CircId) {
        let OptimiserUnit::Circ(Circ::ModuleCirc(main)) = &self.circs[main] else {
            unreachable!()
        };

        let main = main.clone();

        for &dep in main.dependencies.iter() {
            let OptimiserUnit::Circ(Circ::ModuleCirc(_)) = &self.circs[dep] else {
                continue;
            };
            self.generate_truth_table(dep);
        }
    }

    fn generate_truth_table(&mut self, circ_id: CircId) -> bool {
        let OptimiserUnit::Circ(Circ::ModuleCirc(circ)) = &self.circs[circ_id] else {
            unreachable!()
        };

        let circ = circ.clone();

        let stats = Self::get_pin_stats(&circ);

        if stats.input_count > 12 || stats.output_count > 64 {
            return false;
        };

        let mut endpoints = Interner::new();
        let mut bit_connections = HashSet::new();
        let mut dependency_inputs = HashMap::new();
        for connection in circ.connections.iter() {
            if connection.connection_type == ConnectionType::Bidirectional {
                return false;
            };

            let mut source_ids = Vec::with_capacity(circ.connections.len());
            let mut dest_ids = Vec::with_capacity(circ.connections.len());

            match connection.source {
                ConnectionEndpoint::Pin {
                    pin_id,
                    range: ConnectionRange { start, end },
                } => {
                    for bit in start..end {
                        source_ids.push(endpoints.intern(BitEndpoint::Pin { pin_id, bit }));
                    }
                }
                ConnectionEndpoint::Dependency {
                    dependency_id,
                    pin_id,
                    range: ConnectionRange { start, end },
                } => {
                    for bit in start..end {
                        source_ids.push(endpoints.intern(BitEndpoint::Dependency {
                            dependency_id,
                            pin_id,
                            bit,
                        }));
                    }
                }
            }

            match connection.dest {
                ConnectionEndpoint::Pin {
                    pin_id,
                    range: ConnectionRange { start, end },
                } => {
                    for bit in start..end {
                        dest_ids.push(endpoints.intern(BitEndpoint::Pin { pin_id, bit }));
                    }
                }
                ConnectionEndpoint::Dependency {
                    dependency_id,
                    pin_id,
                    range: ConnectionRange { start, end },
                } => {
                    for bit in start..end {
                        let endpoint_id = endpoints.intern(BitEndpoint::Dependency {
                            dependency_id,
                            pin_id,
                            bit,
                        });

                        dest_ids.push(endpoint_id);
                        dependency_inputs
                            .entry(endpoint_id)
                            .or_insert(vec![])
                            .push(*source_ids.last().unwrap())
                    }
                }
            }

            assert_eq!(source_ids.len(), dest_ids.len());

            bit_connections.extend(
                source_ids
                    .into_iter()
                    .zip(dest_ids)
                    .map(|(source, dest)| BitConnection { source, dest }),
            );
        }

        let bit_connections = bit_connections.into_iter().collect::<Vec<_>>();

        let mut endpoint_inbound_connection_ids = HashMap::new();
        let mut output_endpoint_ids = HashSet::new();

        for (id, conn) in bit_connections.iter().enumerate() {
            if let BitEndpoint::Pin { pin_id, .. } = endpoints.get(conn.dest).unwrap() {
                assert!(stats.output_pins.contains(&pin_id));
                output_endpoint_ids.insert(conn.dest);
            }

            endpoint_inbound_connection_ids
                .entry(conn.dest)
                .or_insert(vec![])
                .push(id);
        }

        let mut ctx = TruthTableContext {
            circ: &circ,
            endpoints: &endpoints,
            endpoint_inbound_connection_ids: &endpoint_inbound_connection_ids,
            bit_connections: &bit_connections,
            tree_table_cache: HashMap::new(),
        };

        let mut connection_trees = Vec::with_capacity(output_endpoint_ids.len());
        for output in output_endpoint_ids.into_iter() {
            match self.generate_connection_tree(&ctx, output) {
                Some(tree) => connection_trees.push((output, tree)),
                None => return false,
            }
        }

        let Some(tt) = self.build_truth_table(&mut ctx, connection_trees) else {
            return false;
        };

        self.circs[circ_id] = OptimiserUnit::TruthTable(tt);

        true
    }

    fn build_truth_table(
        &mut self,
        ctx: &mut TruthTableContext,
        connection_trees: Vec<(BitEndpointId, Rc<ConnectionTree>)>,
    ) -> Option<TruthTable> {
        let tree_tables = connection_trees
            .into_iter()
            .map(|(endpoint_id, ct)| {
                let endpoint = ctx.get_endpoint(endpoint_id);
                if let Some(tt) = ctx.tree_table_cache.get(&ct) {
                    Some((*endpoint, tt.clone()))
                } else {
                    Some((*endpoint, self.get_tree_table(ctx, ct.clone())?))
                }
            })
            .collect::<Option<HashMap<BitEndpoint, Rc<TreeTable>>>>()?;

        Some(TruthTable::from_tree_tables(tree_tables))
    }

    fn get_tree_table(
        &mut self,
        ctx: &mut TruthTableContext,
        tree: Rc<ConnectionTree>,
    ) -> Option<Rc<TreeTable>> {
        if let Some(tt) = ctx.tree_table_cache.get(&tree) {
            return Some(tt.clone());
        };

        let tt = match tree.as_ref() {
            ConnectionTree::Source(pin) => TreeTable::identity(*pin),
            ConnectionTree::Combine(trees) => {
                let sub_trees: Vec<Rc<TreeTable>> = trees
                    .iter()
                    .map(|tree| self.get_tree_table(ctx, tree.clone()))
                    .collect::<Option<Vec<_>>>()?;

                TreeTable::combine(&sub_trees)
            }
            ConnectionTree::TruthTable(circ_id, pin) => {
                let OptimiserUnit::TruthTable(tt) = &self.circs[*circ_id] else {
                    unreachable!();
                };

                let tt = tt.get_tree_table(*pin);
                let mut input_mapping = HashMap::new();

                for dep_input in tt.inputs.keys() {
                    let tree = match dep_input {
                        d @ BitEndpoint::Dependency { .. } => self
                            .generate_connection_tree(ctx, ctx.endpoints.get_index(&d).unwrap())?,
                        _ => unreachable!(),
                    };

                    match *tree {
                        ConnectionTree::NotConnected => {
                            input_mapping.insert(*dep_input, None);
                            continue;
                        }
                        _ => (),
                    }

                    let tt = self.get_tree_table(ctx, tree.clone())?;
                    input_mapping.insert(*dep_input, Some(tt));
                }

                tt.map_inputs(input_mapping)
            }
            ConnectionTree::NotConnected => unreachable!(),
        };

        let tt = Rc::new(tt);

        ctx.tree_table_cache.insert(tree.clone(), tt.clone());

        Some(tt)
    }

    // fn get_bit_connections(
    //     endpoints: &Interner<ConnectionEndpoint>,
    //     connections: &[Connection],
    // ) -> Vec<BitConnection> {
    //     connections
    //         .iter()
    //         .enumerate()
    //         .map(|(id, conn)| {
    //             (0..conn.width).map(move |bit| BitConnection {
    //                 id,
    //                 bit,
    //                 source: endpoints.get_index(&conn.source).unwrap(),
    //                 dest: endpoints.get_index(&conn.dest).unwrap(),
    //             })
    //         })
    //         .flatten()
    //         .collect()
    // }

    fn generate_connection_tree(
        &mut self,
        ctx: &TruthTableContext,
        endpoint_id: BitEndpointId,
    ) -> Option<Rc<ConnectionTree>> {
        let mut dependencies = Vec::new();

        for connection in ctx.get_endpoint_inbound_connections(endpoint_id) {
            let sub_tree = match ctx.get_endpoint(connection.source) {
                pin @ BitEndpoint::Pin { .. } => ConnectionTree::Source(*pin),
                pin @ BitEndpoint::Dependency { dependency_id, .. } => {
                    let circ_id = ctx.get_circ_dependency(*dependency_id);
                    if self.get_truth_table(circ_id) {
                        ConnectionTree::TruthTable(circ_id, *pin)
                    } else {
                        return None;
                    }
                }
            };

            dependencies.push(Rc::new(sub_tree));
        }

        Some(match dependencies.len() {
            0 => Rc::new(ConnectionTree::NotConnected),
            1 => dependencies.pop().unwrap(),
            _ => Rc::new(ConnectionTree::Combine(dependencies)),
        })
    }

    fn get_truth_table(&mut self, circ_id: CircId) -> bool {
        if matches!(&self.circs[circ_id], OptimiserUnit::TruthTable(_)) {
            true
        } else if self.attempted.contains(&circ_id) {
            false
        } else if matches!(
            &self.circs[circ_id],
            OptimiserUnit::Circ(Circ::ModuleCirc(_))
        ) {
            self.generate_truth_table(circ_id)
        } else {
            false
        }
    }
}

// struct TruthTableBuilder {
//     input_pins: HashMap<PinId, Pin>,
//     output_pins: HashMap<PinId, Pin>,
// }

// impl TruthTableBuilder {
//     fn new(pins: &OrderedMap<IdentId, Pin>) -> Self {
//         let mut input_pins = HashMap::new();
//         // let mut input_count = 0;
//         let mut output_pins = HashMap::new();
//         // let mut output_count = 0;

//         // pins.value_indices().for_each(|(pin_id, &pin)| if pin.direction == PinDirection::Input {
//         //     input_pins.insert(pin_id, pin);
//         //     input_count += pin.width
//         // })
//         Self {
//             input_pins,
//             output_pins,
//         }
//     }

//     fn direct_connection(&mut self, source: PinBit, dest: PinBit) -> &mut Self {
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {
    use super::BitString;

    #[test]
    fn test_bitstring_serde() {
        let value = 987251230u128;
        let bs = BitString::from_value(value, 16);
        dbg!(&bs);
        assert_eq!(dbg!(bs.value()), value);
    }
}
