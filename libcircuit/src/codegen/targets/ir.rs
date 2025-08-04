use std::collections::{HashMap, HashSet};

use crate::{
    analyser::{Circ, CircId, Connection, ConnectionEndpoint, ConnectionRange, Pin, PinId},
    ast::PinDirection,
    codegen::blocks::{BlockAddress, Blocks},
    optimiser::{BitEndpoint, OptimiserUnit, TruthTable},
    util::{div_up, invert_hashmap},
};

pub type IrBlocks = Blocks<Opcode, usize>;
pub type Register = usize;

#[derive(Debug, Default)]
struct RegisterAllocator {
    next: usize,
}

impl RegisterAllocator {
    fn new() -> Self {
        Self::default()
    }

    fn next(&mut self) -> Register {
        let id = self.next;
        self.next += 1;
        id
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Address {
    Immediate(BlockAddress),
    Register(Register),
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Register(n) => write!(f, "r{n}"),
            Self::Immediate((blk, addr)) => write!(f, "b{blk}:{addr}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Constant {
    Value(usize),
    Address(BlockAddress),
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(v) => write!(f, "{v}"),
            Self::Address((blk, addr)) => write!(f, "b{blk}:{addr}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Value {
    Register(Register),
    Immediate(Constant),
    Indirect(Address),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Register(n) => write!(f, "r{n}"),
            Self::Immediate(i) => write!(f, "{i}"),
            Self::Indirect(addr) => write!(f, "[{addr}]"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum OpcodeKind {
    Add(Register, Value),
    Call(Address),
    Load(Constant),
    Mul(Register, Value),
    Pop,
    Push(Value),
    Read(Address),
    ReadBits(Address, usize, usize),
    ReadOffset(Address, Value),
    Return,
    Write(Address, Value),
    WriteBits(Address, Register, usize, usize),
    WriteOffset(Address, Register, Value),
}

impl std::fmt::Display for OpcodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(v) => write!(f, "{v}"),
            Self::Read(a) => write!(f, "[{a}]"),
            Self::ReadOffset(a, v) => write!(f, "[{a} + {v}]"),
            Self::Write(a, v) => write!(f, "[{a}] <- {v}"),
            Self::WriteOffset(a, r, offset) => write!(f, "[{a} + {offset}] <- r{r}"),
            Self::Mul(r, v) => write!(f, "r{r} * {v}"),
            Self::Add(r, v) => write!(f, "r{r} + {v}"),
            Self::Push(v) => write!(f, "push {v}"),
            Self::Pop => write!(f, "pop"),
            Self::Return => write!(f, "return"),
            Self::Call(a) => write!(f, "call {a}"),
            Self::ReadBits(a, s, e) => write!(f, "[{a} bits {s} up to {e}]"),
            Self::WriteBits(a, r, s, e) => write!(f, "[{a} bits {s} up to {e}] <- r{r}"),
        }
    }
}

#[derive(Debug, Default)]
struct CodeBlockBuilder {
    register_allocator: RegisterAllocator,
    opcodes: Vec<Opcode>,
}

macro_rules! opcode_impls {
    (
        $(
            $opcode:ident,
            $base_name:ident($($arg:ident: $t:ty),*),
            $imm_name:ident,
            $ind_name:ident,
            $reg_name:ident
        );*
        $(;)?
    ) =>{
        impl CodeBlockBuilder {
            $(
                fn $base_name(&mut self, $($arg: $t,)* value: Value) -> Register {
                    let destination = self.register_allocator.next();
                    self.opcodes.push(
                        Opcode {
                            destination,
                            opcode: OpcodeKind::$opcode($($arg,)* value),
                        }
                    );

                    destination
                }

                fn $imm_name(&mut self, $($arg: $t,)* value: usize) -> Register {
                    self.$base_name($($arg,)* Value::Immediate(Constant::Value(value)))
                }

                fn $ind_name(&mut self, $($arg: $t,)* address: Address) -> Register {
                    self.$base_name($($arg,)* Value::Indirect(address))
                }

                fn $reg_name(&mut self, $($arg: $t,)* register: Register) -> Register {
                    self.$base_name($($arg,)* Value::Register(register))
                }
            )*
        }
    };
}

opcode_impls! {
    Write, write(into: Address), write_imm, write_ind, write_reg;
    WriteOffset, write_offset(into: Address, data: Register), write_offset_imm, write_offset_ind, write_offset_reg;
    Mul, mul(lhs: Register), mul_imm, mul_ind, mul_reg;
    Add, add(lhs: Register), add_imm, add_ind, add_reg;
    Push, push(), push_imm, push_ind, push_reg;
}

impl CodeBlockBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn load(&mut self, value: Constant) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Load(value),
        });

        destination
    }

    fn load_const(&mut self, value: usize) -> Register {
        self.load(Constant::Value(value))
    }

    fn load_addr(&mut self, addr: BlockAddress) -> Register {
        self.load(Constant::Address(addr))
    }

    fn read(&mut self, addr: BlockAddress) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Read(Address::Immediate(addr)),
        });

        destination
    }

    fn read_reg(&mut self, register: Register) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Read(Address::Register(register)),
        });

        destination
    }

    fn pop(&mut self) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Pop,
        });

        destination
    }

    fn push_addr(&mut self, addr: BlockAddress) {
        self.push(Value::Immediate(Constant::Address(addr)));
    }

    fn ret(&mut self) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Return,
        });
    }

    fn call(&mut self, addr: Address) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Call(addr),
        });
    }

    fn call_imm(&mut self, addr: BlockAddress) {
        self.call(Address::Immediate(addr))
    }

    fn call_reg(&mut self, register: Register) {
        self.call(Address::Register(register))
    }

    fn read_bits(&mut self, addr: Address, start: usize, end: usize) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::ReadBits(addr, start, end),
        });

        destination
    }

    fn read_bits_imm(&mut self, addr: BlockAddress, start: usize, end: usize) -> Register {
        self.read_bits(Address::Immediate(addr), start, end)
    }

    fn read_bits_reg(&mut self, register: Register, start: usize, end: usize) -> Register {
        self.read_bits(Address::Register(register), start, end)
    }

    fn write_bits(&mut self, addr: Address, register: Register, start: usize, end: usize) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::WriteBits(addr, register, start, end),
        });
    }

    fn write_bits_imm(&mut self, addr: BlockAddress, register: Register, start: usize, end: usize) {
        self.write_bits(Address::Immediate(addr), register, start, end)
    }

    fn write_bits_reg(
        &mut self,
        addr_register: Register,
        data_register: Register,
        start: usize,
        end: usize,
    ) {
        self.write_bits(Address::Register(addr_register), data_register, start, end)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Opcode {
    pub destination: Register,
    pub opcode: OpcodeKind,
}

impl std::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{} = {}", self.destination, self.opcode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CircEndpoint {
    Pin(PinId),
    Dependency(usize, PinId),
}

pub struct IrGen<'a> {
    units: &'a [OptimiserUnit],
    blocks: IrBlocks,
    code_blocks: HashMap<usize, usize>,
}

impl<'a> IrGen<'a> {
    pub fn new(units: &'a [OptimiserUnit]) -> Self {
        Self {
            units,
            blocks: IrBlocks::default(),
            code_blocks: HashMap::new(),
        }
    }

    pub fn emit(mut self, main: CircId) -> IrBlocks {
        let OptimiserUnit::Circ(circ) = &self.units[main] else {
            unreachable!();
        };

        let main_code_block_id = self.emit_circ_blocks(main, circ);
        self.blocks.set_entry((main_code_block_id, 0));

        self.blocks
    }

    fn emit_circ_blocks(&mut self, unit_id: usize, circ: &Circ) -> usize {
        if let Some(&code_block_id) = self.code_blocks.get(&unit_id) {
            return code_block_id;
        };

        let mut dependencies = Vec::with_capacity(circ.dependencies.len());

        for unit_id in circ.dependencies.iter() {
            let code_block_id = match &self.units[*unit_id] {
                OptimiserUnit::Circ(circ) => self.emit_circ_blocks(*unit_id, circ),
                OptimiserUnit::TruthTable(tt) => self.emit_tt_blocks(*unit_id, tt),
            };

            dependencies.push(code_block_id);
        }

        let code_block_id = self.emit_circ_code_block(unit_id, circ);
        self.code_blocks.insert(unit_id, code_block_id);
        code_block_id
    }

    fn get_level(
        &self,
        connections_grouped_by_source: &HashMap<CircEndpoint, Vec<Connection>>,
        endpoint_levels: &mut HashMap<CircEndpoint, usize>,
        connection_levels: &mut HashMap<Connection, usize>,
        dependency_levels: &mut HashMap<usize, usize>,
        circ: &Circ,
        level: usize,
    ) {
        assert!(level > 0);

        let dependencies_to_check = endpoint_levels
            .iter()
            .filter_map(|(e, &l)| match e {
                CircEndpoint::Dependency(dep_id, _) => (l == level - 1).then_some(*dep_id),
                _ => None,
            })
            .collect::<HashSet<_>>();

        if dependencies_to_check.is_empty() {
            return;
        }

        for dependency in dependencies_to_check {
            let circ_id = circ.dependencies[dependency];
            let (dep_inputs, dep_outputs) = match &self.units[circ_id] {
                OptimiserUnit::Circ(c) => (
                    c.pins
                        .value_indices()
                        .filter_map(|(i, p)| {
                            (p.direction == PinDirection::Input)
                                .then_some(CircEndpoint::Dependency(dependency, i))
                        })
                        .collect::<HashSet<CircEndpoint>>(),
                    c.pins
                        .value_indices()
                        .filter_map(|(i, p)| {
                            (p.direction == PinDirection::Output)
                                .then_some(CircEndpoint::Dependency(dependency, i))
                        })
                        .collect::<HashSet<CircEndpoint>>(),
                ),
                OptimiserUnit::TruthTable(tt) => (
                    tt.inputs
                        .keys()
                        .map(|e| match e {
                            BitEndpoint::Pin { pin_id, .. } => {
                                CircEndpoint::Dependency(dependency, *pin_id)
                            }
                            _ => unreachable!(),
                        })
                        .collect::<HashSet<CircEndpoint>>(),
                    tt.outputs
                        .keys()
                        .map(|e| match e {
                            BitEndpoint::Pin { pin_id, .. } => {
                                CircEndpoint::Dependency(dependency, *pin_id)
                            }
                            _ => unreachable!(),
                        })
                        .collect::<HashSet<CircEndpoint>>(),
                ),
            };

            let dep_level = dep_inputs
                .into_iter()
                .map(|input| endpoint_levels.get(&input))
                .try_fold(0, |acc, level| Some(std::cmp::max(acc, *level?)));
            let Some(dep_level) = dep_level else { continue };

            dependency_levels.insert(dependency, dep_level + 1);

            let connections = dep_outputs
                .iter()
                .map(|endpoint| connections_grouped_by_source[&endpoint].iter())
                .flatten();
            connection_levels.extend(
                connections
                    .clone()
                    .copied()
                    .zip(std::iter::repeat(dep_level + 1)),
            );
            endpoint_levels.extend(connections.map(|conn| {
                (
                    match conn.dest {
                        ConnectionEndpoint::Pin { pin_id, .. } => CircEndpoint::Pin(pin_id),
                        ConnectionEndpoint::Dependency {
                            dependency_id,
                            pin_id,
                            ..
                        } => CircEndpoint::Dependency(dependency_id, pin_id),
                    },
                    dep_level + 1,
                )
            }));
        }

        self.get_level(
            connections_grouped_by_source,
            endpoint_levels,
            connection_levels,
            dependency_levels,
            circ,
            level + 1,
        )
    }

    fn levelise_connections(
        &self,
        connections_grouped_by_source: HashMap<CircEndpoint, Vec<Connection>>,
        circ: &Circ,
    ) -> (HashMap<Connection, usize>, HashMap<usize, usize>) {
        let mut connection_levels = circ
            .pins
            .value_indices()
            .filter_map(|(id, p)| {
                (p.direction == PinDirection::Input).then_some(CircEndpoint::Pin(id))
            })
            .map(|endpoint| connections_grouped_by_source[&endpoint].iter().copied())
            .flatten()
            .zip(std::iter::repeat(0))
            .collect::<HashMap<_, _>>();

        let mut endpoint_levels = connection_levels
            .keys()
            .filter_map(|conn| match conn.dest {
                ConnectionEndpoint::Dependency {
                    dependency_id,
                    pin_id,
                    ..
                } => Some(CircEndpoint::Dependency(dependency_id, pin_id)),
                _ => None,
            })
            .zip(std::iter::repeat(0))
            .collect::<HashMap<_, _>>();

        let mut dependency_levels = circ
            .dependencies
            .iter()
            .enumerate()
            .filter_map(|(dep_id, circ_id)| {
                match &self.units[*circ_id] {
                    OptimiserUnit::Circ(c) => c
                        .pins
                        .value_indices()
                        .filter_map(|(i, p)| {
                            (p.direction == PinDirection::Input)
                                .then_some(CircEndpoint::Dependency(dep_id, i))
                        })
                        .next()
                        .is_none(),
                    _ => false,
                }
                .then_some((dep_id, 0))
            })
            .collect::<HashMap<_, _>>();

        self.get_level(
            &connections_grouped_by_source,
            &mut endpoint_levels,
            &mut connection_levels,
            &mut dependency_levels,
            circ,
            1,
        );

        (connection_levels, dependency_levels)
    }

    fn emit_circ_code_block(&mut self, unit_id: usize, circ: &Circ) -> usize {
        let connections_grouped_by_source = circ
            .connections
            .iter()
            .map(|conn| match conn.source {
                ConnectionEndpoint::Pin { pin_id, .. } => (CircEndpoint::Pin(pin_id), *conn),
                ConnectionEndpoint::Dependency {
                    dependency_id,
                    pin_id,
                    ..
                } => (CircEndpoint::Dependency(dependency_id, pin_id), *conn),
            })
            .fold(HashMap::new(), |mut hm, (k, v)| {
                hm.entry(k).or_insert(vec![]).push(v);
                hm
            });

        let (connection_levels, dependency_levels) =
            self.levelise_connections(connections_grouped_by_source, circ);

        if (0..circ.dependencies.len()).any(|d| !dependency_levels.contains_key(&d)) {
            todo!("handle logic loops")
        }

        let mut connection_levels = invert_hashmap(connection_levels);
        let mut dependency_levels = invert_hashmap(dependency_levels);

        let mut all_levels: Vec<usize> = connection_levels
            .keys()
            .copied()
            .chain(dependency_levels.keys().copied())
            .collect();
        all_levels.sort();
        all_levels.dedup();

        let processing_order = all_levels
            .into_iter()
            .map(|level| {
                (
                    dependency_levels.remove(&level).unwrap_or(vec![]),
                    connection_levels.remove(&level).unwrap_or(vec![]),
                )
            })
            .collect::<Vec<(_, _)>>();

        let mut code_block = CodeBlockBuilder::new();

        let circ_data_ptr = code_block.pop();
        let flags_addr = code_block.add_imm(circ_data_ptr, Self::get_flags_offset());
        // clear dirty bit in flags
        code_block.write_imm(Address::Register(flags_addr), 0);

        let circ_ptrs_addr = code_block.add_imm(circ_data_ptr, Self::get_circ_ptrs_offset(circ));

        for (dependencies_to_update, connections_to_process) in processing_order {
            for dependency_id in dependencies_to_update {
                let dep_resv = code_block.add_imm(circ_ptrs_addr, dependency_id);
                code_block.push_ind(Address::Register(dep_resv));
                let circ_id = circ.dependencies[dependency_id];
                let code_block_id = self.code_blocks[&circ_id];
                code_block.call(Address::Immediate((code_block_id, 0)));
            }

            for connection in connections_to_process {
                let source_reg = match connection.source {
                    ConnectionEndpoint::Pin {
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        let r0 = code_block.add_imm(circ_data_ptr, word_start);
                        code_block.read_bits_reg(r0, bit_start + start, bit_start + end)
                    }
                    ConnectionEndpoint::Dependency {
                        dependency_id,
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let dep_resv_ptr = code_block.add_imm(circ_ptrs_addr, dependency_id);
                        let unit_id = circ.dependencies[dependency_id];
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        let r0 = code_block.add_imm(dep_resv_ptr, word_start);
                        code_block.read_bits_reg(r0, bit_start + start, bit_start + end)
                    }
                };

                match connection.dest {
                    ConnectionEndpoint::Pin {
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        let r0 = code_block.add_imm(circ_data_ptr, word_start);
                        code_block.write_bits_reg(
                            r0,
                            source_reg,
                            bit_start + start,
                            bit_start + end,
                        );
                    }
                    ConnectionEndpoint::Dependency {
                        dependency_id,
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let dep_resv_ptr = code_block.add_imm(circ_ptrs_addr, dependency_id);
                        let unit_id = circ.dependencies[dependency_id];
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        let r0 = code_block.add_imm(dep_resv_ptr, word_start);
                        code_block.write_bits_reg(
                            r0,
                            source_reg,
                            bit_start + start,
                            bit_start + end,
                        );
                    }
                }
            }
        }

        code_block.ret();

        let circ_code = self.blocks.new_code();
        circ_code.extend(&code_block.opcodes);

        circ_code.id
    }

    fn get_pin_offset(&self, unit_id: usize, pin_id: PinId) -> (usize, usize) {
        let unit = &self.units[unit_id];
        match unit {
            OptimiserUnit::Circ(circ) => {
                let pin = circ.pins.get_by_index(pin_id).unwrap();
                match pin {
                    Pin {
                        direction: PinDirection::Input,
                        ..
                    } => {
                        let pin_starts = circ
                            .pins
                            .value_indices()
                            .filter(|(_, p)| p.direction == PinDirection::Input)
                            .map(|(id, p)| std::iter::repeat_n(id, p.width).enumerate())
                            .flatten()
                            .enumerate()
                            .filter_map(|(start, (i, pin_id))| (i == 0).then_some((pin_id, start)))
                            .collect::<HashMap<PinId, usize>>();

                        let bit_offset = 2 * pin_starts[&pin_id];
                        let word_offset = bit_offset / usize::BITS as usize;
                        let bit_offset = (bit_offset % usize::BITS as usize) / 2;

                        (Self::get_input_offset() + word_offset, bit_offset)
                    }
                    Pin {
                        direction: PinDirection::Output,
                        ..
                    } => {
                        let pin_starts = circ
                            .pins
                            .value_indices()
                            .filter(|(_, p)| p.direction == PinDirection::Output)
                            .map(|(id, p)| std::iter::repeat_n(id, p.width).enumerate())
                            .flatten()
                            .enumerate()
                            .filter_map(|(start, (i, pin_id))| (i == 0).then_some((pin_id, start)))
                            .collect::<HashMap<PinId, usize>>();

                        let bit_offset = 2 * pin_starts[&pin_id];
                        let word_offset = bit_offset / usize::BITS as usize;
                        let bit_offset = (bit_offset % usize::BITS as usize) / 2;

                        (Self::get_circ_output_offset(circ) + word_offset, bit_offset)
                    }
                    _ => todo!("handle transputs"),
                }
            }
            OptimiserUnit::TruthTable(tt) => {
                let (bit_offset, direction) = tt
                    .inputs
                    .iter()
                    .filter_map(|(e, s)| match e {
                        BitEndpoint::Pin { pin_id: p, .. } => (*p == pin_id).then_some(s),
                        _ => unreachable!(),
                    })
                    .zip(std::iter::repeat(PinDirection::Input))
                    .min_by_key(|(x, _)| **x)
                    .or(tt
                        .outputs
                        .iter()
                        .filter_map(|(e, s)| match e {
                            BitEndpoint::Pin { pin_id: p, .. } => (*p == pin_id).then_some(s),
                            _ => unreachable!(),
                        })
                        .zip(std::iter::repeat(PinDirection::Output))
                        .min_by_key(|(x, _)| **x))
                    .unwrap();

                let bit_offset = 2 * bit_offset;
                let word_offset = bit_offset / usize::BITS as usize;
                let bit_offset = (bit_offset % usize::BITS as usize) / 2;

                (
                    match direction {
                        PinDirection::Input => Self::get_input_offset(),
                        PinDirection::Output => Self::get_tt_output_offset(tt),
                        _ => unreachable!(),
                    } + word_offset,
                    bit_offset,
                )
            }
        }
    }

    fn get_circ_inputs_size(circ: &Circ) -> usize {
        div_up(
            2 * circ
                .pins
                .values()
                .filter_map(|p| {
                    if p.direction == PinDirection::Input {
                        Some(p.width)
                    } else {
                        None
                    }
                })
                .sum::<usize>(),
            usize::BITS as usize,
        )
    }

    fn get_circ_output_offset(circ: &Circ) -> usize {
        Self::get_input_offset() + Self::get_circ_inputs_size(circ)
    }

    fn get_circ_outputs_size(circ: &Circ) -> usize {
        div_up(
            2 * circ
                .pins
                .values()
                .filter_map(|p| {
                    if p.direction == PinDirection::Output {
                        Some(p.width)
                    } else {
                        None
                    }
                })
                .sum::<usize>(),
            usize::BITS as usize,
        )
    }

    fn get_circ_ptrs_offset(circ: &Circ) -> usize {
        Self::get_circ_output_offset(circ) + Self::get_circ_outputs_size(circ)
    }

    // fn emit_circ_resv_block(&mut self, circ: &Circ) -> usize {
    //     let circ_resv = self.blocks.new_resv(format!("circ_resv"));

    //     assert!(circ_resv.offset() == Self::get_flags_offset());
    //     circ_resv.reserve(Self::get_flags_size());

    //     assert!(circ_resv.offset() == Self::get_input_offset());
    // }

    fn get_flags_offset() -> usize {
        0
    }

    fn get_flags_size() -> usize {
        1
    }

    fn get_input_offset() -> usize {
        Self::get_flags_offset() + Self::get_flags_size()
    }

    fn get_tt_inputs_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.inputs.len(), usize::BITS as usize)
    }

    fn get_tt_row_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.outputs.len(), usize::BITS as usize)
    }

    fn get_tt_output_offset(tt: &TruthTable) -> usize {
        Self::get_input_offset() + Self::get_tt_inputs_size(tt)
    }

    fn emit_tt_code_block(&mut self, tt: &TruthTable, tt_data_block: usize) -> usize {
        // TODO: only emit a single code block that processes all TTs

        let mut code_block = CodeBlockBuilder::new();

        let tt_data_ptr = code_block.pop();
        let flags_addr = code_block.add_imm(tt_data_ptr, Self::get_flags_offset());
        // clear dirty bit in flags
        code_block.write_imm(Address::Register(flags_addr), 0);

        let inputs_addr = code_block.add_imm(tt_data_ptr, Self::get_input_offset());
        let outputs_addr = code_block.add_imm(tt_data_ptr, Self::get_tt_output_offset(tt));

        assert_eq!(Self::get_tt_inputs_size(tt), 1);

        let row_size = Self::get_tt_row_size(tt);

        let r0 = code_block.read_reg(inputs_addr);
        let r1 = code_block.mul_imm(r0, row_size);

        let tt_data_address = self.blocks.get_data_block(tt_data_block).base_addr();
        let r2 = code_block.load_addr(tt_data_address);
        let r3 = code_block.add_reg(r2, r1);

        for i in 0..row_size {
            let r4 = code_block.add_imm(r3, i);
            let r5 = code_block.read_reg(r4);
            code_block.write_offset_imm(Address::Register(outputs_addr), r5, i);
        }

        code_block.ret();

        let tt_code = self.blocks.new_code();
        tt_code.extend(&code_block.opcodes);

        tt_code.id
    }

    fn emit_tt_data_block(&mut self, tt: &TruthTable) -> usize {
        let tt_data = self.blocks.new_data();
        let row_size = Self::get_tt_row_size(tt);

        const SIZEOF_USIZE: usize = std::mem::size_of::<usize>();

        for row in tt.iter_rows() {
            let bytes = row.value().to_ne_bytes();
            for i in 0..row_size {
                let index = i * SIZEOF_USIZE;
                let mut byte_array = [0u8; SIZEOF_USIZE];
                byte_array.copy_from_slice(&bytes[index..index + SIZEOF_USIZE]);
                tt_data.push(usize::from_ne_bytes(byte_array));
            }
        }

        dbg!(&tt_data);

        tt_data.id
    }

    fn emit_tt_resv_block(&mut self, tt: &TruthTable) -> usize {
        let tt_resv = self.blocks.new_resv();

        assert!(tt_resv.offset() == Self::get_flags_offset());
        tt_resv.reserve(Self::get_flags_size());

        assert!(tt_resv.offset() == Self::get_input_offset());
        tt_resv.reserve(Self::get_tt_inputs_size(tt));

        assert!(tt_resv.offset() == Self::get_tt_output_offset(tt));
        tt_resv.reserve(Self::get_tt_row_size(tt));

        tt_resv.id
    }

    fn emit_tt_blocks(&mut self, unit_id: usize, tt: &TruthTable) -> usize {
        if let Some(&code_block_id) = self.code_blocks.get(&unit_id) {
            return code_block_id;
        }

        let data_block_id = self.emit_tt_data_block(tt);
        let code_block_id = self.emit_tt_code_block(tt, data_block_id);
        self.code_blocks.insert(unit_id, code_block_id);

        code_block_id
    }
}
