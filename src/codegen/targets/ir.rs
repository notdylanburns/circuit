use std::collections::{HashMap, HashSet};

use crate::{
    analyser::{Circ, CircId, Connection, ConnectionEndpoint, ConnectionRange, Pin, PinId},
    ast::PinDirection,
    codegen::blocks::{Block, BlockAddress, Blocks},
    optimiser::{BitEndpoint, OptimiserUnit, TruthTable},
    util::{div_up, invert_hashmap},
};

pub type Word = usize;
pub type IrBlocks = Blocks<Opcode, Word, ResvItem>;
pub type Register = usize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ResvItem {
    BlockAddress(BlockAddress),
    Word(Word),
}

impl Default for ResvItem {
    fn default() -> Self {
        Self::Word(0)
    }
}

impl std::fmt::Display for ResvItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlockAddress((blk, addr)) => write!(f, "b{blk}:{addr}"),
            Self::Word(x) => write!(f, "{x}"),
        }
    }
}

#[derive(Debug, Default)]
struct RegisterAllocator {
    next: usize,
}

impl RegisterAllocator {
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
    Value(Word),
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
    Call(Address, Value),
    CheckInputs(Register, usize, usize),
    Load(Constant),
    LoadResv,
    Offset {
        base: Address,
        item_size: usize,
        item_count: Value,
    },
    Read(Address),
    ReadBits(Address, usize, usize),
    ReadOffset(Address, usize),
    WriteBits(Address, Register, usize, usize),
    WriteOffset(Address, Value, usize),
}

impl std::fmt::Display for OpcodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call(a, v) => write!(f, "call {a}({v})"),
            Self::CheckInputs(r, a, b) => write!(f, "checkinputs r{r}[{a}] == r{r}[{b}]"),
            Self::Load(v) => write!(f, "{v}"),
            Self::LoadResv => write!(f, "loadresv"),
            Self::Offset {
                base,
                item_size,
                item_count,
            } => write!(f, "offset {base} + {item_size} * {item_count}"),
            Self::Read(a) => write!(f, "[{a}]"),
            Self::ReadBits(a, s, e) => write!(f, "[{a} bits {s} up to {e}]"),
            Self::ReadOffset(a, v) => write!(f, "[{a} + {v}]"),
            Self::WriteBits(a, r, s, e) => write!(f, "[{a} bits {s} up to {e}] <- r{r}"),
            Self::WriteOffset(a, r, offset) => write!(f, "[{a} + {offset}] <- r{r}"),
        }
    }
}

struct CircResvBlock {
    inputs: usize,
    inputs_size: usize,
    inputs_old: usize,
    inputs_old_size: usize,
    outputs: usize,
    outputs_size: usize,
    dependency_ptrs: usize,
    dependency_ptrs_size: usize,
}

impl CircResvBlock {
    fn new(circ: &Circ) -> Self {
        Self {
            inputs: Self::get_input_offset(),
            inputs_size: Self::get_input_size(circ),
            inputs_old: Self::get_old_input_offset(circ),
            inputs_old_size: Self::get_input_size(circ),
            outputs: Self::get_output_offset(circ),
            outputs_size: Self::get_output_size(circ),
            dependency_ptrs: Self::get_dependency_ptrs_offset(circ),
            dependency_ptrs_size: circ.dependencies.len(),
        }
    }

    fn get_input_offset() -> usize {
        0
    }

    fn get_input_size(circ: &Circ) -> usize {
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
            Word::BITS as usize,
        )
    }

    fn get_old_input_offset(circ: &Circ) -> usize {
        Self::get_input_offset() + Self::get_input_size(circ)
    }

    fn get_output_offset(circ: &Circ) -> usize {
        Self::get_old_input_offset(circ) + Self::get_input_size(circ)
    }

    fn get_output_size(circ: &Circ) -> usize {
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
            Word::BITS as usize,
        )
    }

    fn get_dependency_ptrs_offset(circ: &Circ) -> usize {
        Self::get_output_offset(circ) + Self::get_output_size(circ)
    }
}

struct TtResvBlock {
    inputs: usize,
    inputs_size: usize,
    inputs_old: usize,
    inputs_old_size: usize,
    outputs: usize,
    outputs_size: usize,
}

impl TtResvBlock {
    fn new(tt: &TruthTable) -> Self {
        Self {
            inputs: Self::get_input_offset(),
            inputs_size: Self::get_input_size(tt),
            inputs_old: Self::get_old_input_offset(tt),
            inputs_old_size: Self::get_input_size(tt),
            outputs: Self::get_output_offset(tt),
            outputs_size: Self::get_output_size(tt),
        }
    }

    fn get_input_offset() -> usize {
        0
    }

    fn get_input_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.inputs.len(), Word::BITS as usize)
    }

    fn get_old_input_offset(tt: &TruthTable) -> usize {
        Self::get_input_offset() + Self::get_input_size(tt)
    }

    fn get_output_offset(tt: &TruthTable) -> usize {
        Self::get_old_input_offset(tt) + Self::get_input_size(tt)
    }

    fn get_output_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.outputs.len(), Word::BITS as usize)
    }
}

#[derive(Debug, Default)]
struct CodeBlockBuilder {
    register_allocator: RegisterAllocator,
    opcodes: Vec<Opcode>,
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

    fn check_inputs(
        &mut self,
        resv_block_addr: Register,
        inputs_offset: usize,
        old_inputs_offset: usize,
    ) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::CheckInputs(resv_block_addr, inputs_offset, old_inputs_offset),
        });
    }

    fn offset(&mut self, base_addr: Address, item_size: usize, item_count: Value) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Offset {
                base: base_addr,
                item_size,
                item_count,
            },
        });

        destination
    }

    fn offset_addr(
        &mut self,
        address: BlockAddress,
        item_size: usize,
        item_count: Value,
    ) -> Register {
        self.offset(Address::Immediate(address), item_size, item_count)
    }

    fn offset_reg(
        &mut self,
        base_register: Register,
        item_size: usize,
        item_count: Value,
    ) -> Register {
        self.offset(Address::Register(base_register), item_size, item_count)
    }

    fn offset_reg_imm(
        &mut self,
        base_register: Register,
        item_size: usize,
        item_count: usize,
    ) -> Register {
        self.offset(
            Address::Register(base_register),
            item_size,
            Value::Immediate(Constant::Value(item_count)),
        )
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

    fn read_offset(&mut self, addr: Address, offset: usize) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::ReadOffset(addr, offset),
        });

        destination
    }

    fn read_offset_reg(&mut self, addr: Register, offset: usize) -> Register {
        self.read_offset(Address::Register(addr), offset)
    }

    fn load_resv(&mut self) -> Register {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::LoadResv,
        });

        destination
    }

    fn call(&mut self, addr: Address, arg: Value) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Call(addr, arg),
        });
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

    fn write_offset(&mut self, addr: Address, data: Value, offset: usize) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::WriteOffset(addr, data, offset),
        });
    }

    fn write_offset_imm(&mut self, addr: Address, data: Word, offset: usize) {
        self.write_offset(addr, Value::Immediate(Constant::Value(data)), offset);
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

        let (main_code_block_id, main_resv_block_id) = self.emit_circ_blocks(main, circ);
        self.blocks.set_entry((main_code_block_id, 0));
        self.blocks.set_main_resv((main_resv_block_id, 0));

        self.blocks
    }

    fn emit_circ_blocks(&mut self, unit_id: usize, circ: &Circ) -> (usize, usize) {
        let resv_block_id = self.emit_circ_resv_block(circ);

        if let Some(&code_block_id) = self.code_blocks.get(&unit_id) {
            return (code_block_id, resv_block_id);
        };

        let code_block_id = self.emit_circ_code_block(unit_id, circ);
        self.code_blocks.insert(unit_id, code_block_id);

        (code_block_id, resv_block_id)
    }

    fn emit_circ_resv_block(&mut self, circ: &Circ) -> usize {
        let offsets = CircResvBlock::new(circ);

        let dependency_resv_blocks = circ.dependencies.iter().map(|&unit_id|
            match &self.units[unit_id] {
                OptimiserUnit::Circ(circ) => self.emit_circ_blocks(unit_id, circ),
                OptimiserUnit::TruthTable(tt) => self.emit_tt_blocks(unit_id, tt),
            }.1
        ).collect::<Vec<_>>();

        let circ_resv = self.blocks.new_resv();

        assert_eq!(circ_resv.offset(), offsets.inputs);
        circ_resv.reserve(offsets.inputs_size);

        assert_eq!(circ_resv.offset(), offsets.inputs_old);
        circ_resv.reserve(offsets.inputs_old_size);

        assert_eq!(circ_resv.offset(), offsets.outputs);
        circ_resv.reserve(offsets.outputs_size);

        assert_eq!(circ_resv.offset(), offsets.dependency_ptrs);

        for block_id in dependency_resv_blocks {
            circ_resv.push(ResvItem::BlockAddress((block_id, 0)));
        }

        circ_resv.id
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

        let offsets = CircResvBlock::new(circ);

        let mut code_block = CodeBlockBuilder::new();

        let resv_ptr = code_block.load_resv();

        code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);

        let dependency_ptrs = code_block.offset_reg_imm(resv_ptr, 1, offsets.dependency_ptrs);

        for (dependencies_to_update, connections_to_process) in processing_order {
            for dependency_id in dependencies_to_update {
                let dep_resv = code_block.offset_reg_imm(dependency_ptrs, 1, dependency_id);
                let circ_id = circ.dependencies[dependency_id];
                let code_block_id = self.code_blocks[&circ_id];
                code_block.call(
                    Address::Immediate((code_block_id, 0)),
                    Value::Indirect(Address::Register(dep_resv)),
                );
            }

            for connection in connections_to_process {
                let (src_base, mut src_word, src_start, src_end) = match connection.source {
                    ConnectionEndpoint::Pin {
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        (resv_ptr, word_start, bit_start + start, bit_start + end)
                    }
                    ConnectionEndpoint::Dependency {
                        dependency_id,
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let dep_resv = code_block
                            .read_offset(Address::Register(dependency_ptrs), dependency_id);
                        let unit_id = circ.dependencies[dependency_id];
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        (dep_resv, word_start, bit_start + start, bit_start + end)
                    }
                };

                let (dest_base, mut dest_word, dest_start, dest_end) = match connection.dest {
                    ConnectionEndpoint::Pin {
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        (resv_ptr, word_start, bit_start + start, bit_start + end)
                    }
                    ConnectionEndpoint::Dependency {
                        dependency_id,
                        pin_id,
                        range: ConnectionRange { start, end },
                    } => {
                        let dep_resv = code_block
                            .read_offset(Address::Register(dependency_ptrs), dependency_id);
                        let unit_id = circ.dependencies[dependency_id];
                        let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
                        (dep_resv, word_start, bit_start + start, bit_start + end)
                    }
                };

                const WORD_BITS: usize = Word::BITS as usize / 2;

                let mut src_index = src_start;
                let mut dest_index = dest_start;
                loop {
                    let bits_to_write = std::cmp::min(
                        std::cmp::min(WORD_BITS - (src_index % WORD_BITS), src_end - src_index),
                        std::cmp::min(WORD_BITS - (dest_index % WORD_BITS), dest_end - dest_index),
                    );

                    let r_src_word = code_block.offset_reg_imm(src_base, 1, src_word);
                    let r_dest_word = code_block.offset_reg_imm(dest_base, 1, dest_word);

                    let r0 =
                        code_block.read_bits_reg(r_src_word, src_index, src_index + bits_to_write);
                    code_block.write_bits_reg(
                        r_dest_word,
                        r0,
                        dest_index,
                        dest_index + bits_to_write,
                    );

                    src_index += bits_to_write;
                    dest_index += bits_to_write;

                    if src_index == src_end {
                        assert_eq!(dest_index, dest_end);
                        break;
                    }

                    if src_index % WORD_BITS == 0 {
                        src_word += 1;
                    }

                    if dest_index % WORD_BITS == 0 {
                        dest_word += 1;
                    }
                }
            }
        }

        // let circ_ptrs_addr =
        //     code_block.add_imm(circ_data_ptr, Self::get_circ_ptrs_offset(circ) as Word);

        // for (dependencies_to_update, connections_to_process) in processing_order {
        //     for dependency_id in dependencies_to_update {
        //         let dep_resv = code_block.add_imm(circ_ptrs_addr, dependency_id as Word);
        //         let circ_id = circ.dependencies[dependency_id];
        //         let code_block_id = self.code_blocks[&circ_id];
        //         code_block.call(
        //             Address::Immediate((code_block_id, 0)),
        //             Value::Indirect(Address::Register(dep_resv)),
        //         );
        //     }

        //     for connection in connections_to_process {
        //         let (src_base, mut src_word, src_start, src_end) = match connection.source {
        //             ConnectionEndpoint::Pin {
        //                 pin_id,
        //                 range: ConnectionRange { start, end },
        //             } => {
        //                 let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
        //                 (
        //                     circ_data_ptr,
        //                     word_start,
        //                     bit_start + start,
        //                     bit_start + end,
        //                 )
        //             }
        //             ConnectionEndpoint::Dependency {
        //                 dependency_id,
        //                 pin_id,
        //                 range: ConnectionRange { start, end },
        //             } => {
        //                 let dep_resv_ptr =
        //                     code_block.add_imm(circ_ptrs_addr, dependency_id as Word);
        //                 let unit_id = circ.dependencies[dependency_id];
        //                 let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
        //                 (dep_resv_ptr, word_start, bit_start + start, bit_start + end)
        //             }
        //         };

        //         let (dest_base, mut dest_word, dest_start, dest_end) = match connection.dest {
        //             ConnectionEndpoint::Pin {
        //                 pin_id,
        //                 range: ConnectionRange { start, end },
        //             } => {
        //                 let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
        //                 (
        //                     circ_data_ptr,
        //                     word_start,
        //                     bit_start + start,
        //                     bit_start + end,
        //                 )
        //             }
        //             ConnectionEndpoint::Dependency {
        //                 dependency_id,
        //                 pin_id,
        //                 range: ConnectionRange { start, end },
        //             } => {
        //                 let dep_resv_ptr =
        //                     code_block.add_imm(circ_ptrs_addr, dependency_id as Word);
        //                 let unit_id = circ.dependencies[dependency_id];
        //                 let (word_start, bit_start) = self.get_pin_offset(unit_id, pin_id);
        //                 (dep_resv_ptr, word_start, bit_start + start, bit_start + end)
        //             }
        //         };

        //         const WORD_BITS: usize = Word::BITS as usize / 2;

        //         let mut src_index = src_start;
        //         let mut dest_index = dest_start;
        //         loop {
        //             let bits_to_write = std::cmp::min(
        //                 std::cmp::min(WORD_BITS - (src_index % WORD_BITS), src_end - src_index),
        //                 std::cmp::min(WORD_BITS - (dest_index % WORD_BITS), dest_end - dest_index),
        //             );

        //             let r_src_word = code_block.add_imm(src_base, src_word as Word);
        //             let r_dest_word = code_block.add_imm(dest_base, dest_word as Word);

        //             let r0 =
        //                 code_block.read_bits_reg(r_src_word, src_index, src_index + bits_to_write);
        //             code_block.write_bits_reg(
        //                 r_dest_word,
        //                 r0,
        //                 dest_index,
        //                 dest_index + bits_to_write,
        //             );

        //             src_index += bits_to_write;
        //             dest_index += bits_to_write;

        //             if src_index == src_end {
        //                 assert_eq!(dest_index, dest_end);
        //                 break;
        //             }

        //             if src_index % WORD_BITS == 0 {
        //                 src_word += 1;
        //             }

        //             if dest_index % WORD_BITS == 0 {
        //                 dest_word += 1;
        //             }
        //         }
        //     }
        // }

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
                        let word_offset = bit_offset / Word::BITS as usize;
                        let bit_offset = (bit_offset % Word::BITS as usize) / 2;

                        (CircResvBlock::get_input_offset() + word_offset, bit_offset)
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
                        let word_offset = bit_offset / Word::BITS as usize;
                        let bit_offset = (bit_offset % Word::BITS as usize) / 2;

                        (
                            CircResvBlock::get_output_offset(circ) + word_offset,
                            bit_offset,
                        )
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
                let word_offset = bit_offset / Word::BITS as usize;
                let bit_offset = (bit_offset % Word::BITS as usize) / 2;

                (
                    match direction {
                        PinDirection::Input => TtResvBlock::get_input_offset(),
                        PinDirection::Output => TtResvBlock::get_output_offset(tt),
                        _ => unreachable!(),
                    } + word_offset,
                    bit_offset,
                )
            }
        }
    }

    fn emit_tt_code_block(&mut self, tt: &TruthTable, tt_data_block: usize) -> usize {
        let offsets = TtResvBlock::new(tt);

        let mut code_block = CodeBlockBuilder::new();

        let resv_ptr = code_block.load_resv();
        code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);

        assert_eq!(offsets.inputs_size, 1);

        let r0 = code_block.read_offset(Address::Register(resv_ptr), offsets.inputs);

        let data_ptr = self.blocks.get_data_block(tt_data_block).base_addr();
        let r1 = code_block.offset(
            Address::Immediate(data_ptr),
            offsets.outputs_size,
            Value::Register(r0),
        );

        let outputs_addr = code_block.offset_reg_imm(resv_ptr, 1, offsets.outputs);

        for i in 0..offsets.outputs_size {
            let r2 = code_block.read_offset(Address::Register(r1), i);
            code_block.write_offset(Address::Register(outputs_addr), Value::Register(r2), i);
        }

        let tt_code = self.blocks.new_code();
        tt_code.extend(&code_block.opcodes);

        tt_code.id
    }

    fn emit_tt_data_block(&mut self, tt: &TruthTable) -> usize {
        let offsets = TtResvBlock::new(tt);

        let tt_data = self.blocks.new_data();

        const SIZEOF_WORD: usize = std::mem::size_of::<Word>();

        for row in tt.iter_rows() {
            let bytes = row.value().to_ne_bytes();
            for i in 0..offsets.outputs_size {
                let index = i * SIZEOF_WORD;
                let mut byte_array = [0u8; SIZEOF_WORD];
                byte_array.copy_from_slice(&bytes[index..index + SIZEOF_WORD]);
                tt_data.push(Word::from_ne_bytes(byte_array));
            }
        }

        tt_data.id
    }

    fn emit_tt_resv_block(&mut self, tt: &TruthTable) -> usize {
        let offsets = TtResvBlock::new(tt);

        let tt_resv = self.blocks.new_resv();

        assert!(tt_resv.offset() == offsets.inputs);
        tt_resv.reserve(offsets.inputs_size);

        assert!(tt_resv.offset() == offsets.inputs_old);
        tt_resv.reserve(offsets.inputs_old_size);

        assert!(tt_resv.offset() == offsets.outputs);
        tt_resv.reserve(offsets.outputs_size);

        tt_resv.id
    }

    fn emit_tt_blocks(&mut self, unit_id: usize, tt: &TruthTable) -> (usize, usize) {
        let resv_block_id = self.emit_tt_resv_block(tt);

        if let Some(&code_block_id) = self.code_blocks.get(&unit_id) {
            return (code_block_id, resv_block_id);
        }

        let data_block_id = self.emit_tt_data_block(tt);
        let code_block_id = self.emit_tt_code_block(tt, data_block_id);
        self.code_blocks.insert(unit_id, code_block_id);

        (code_block_id, resv_block_id)
    }
}

pub fn get_register_usage(block: &Block<Opcode>) -> Vec<Vec<bool>> {
    block
        .iter()
        .map(|opcode| register_is_used(block, opcode.destination))
        .collect()
}

fn register_is_used(block: &Block<Opcode>, register: Register) -> Vec<bool> {
    block
        .items()
        .iter()
        .map(|opcode| {
            if opcode.destination == register {
                false
            } else {
                match opcode.opcode {
                    OpcodeKind::Call(a, v) => {
                        register_used_in_address(register, a) || register_used_in_value(register, v)
                    }
                    OpcodeKind::CheckInputs(r, _, _) => r == register,
                    OpcodeKind::Load(_) => false,
                    OpcodeKind::LoadResv => false,
                    OpcodeKind::Offset {
                        base, item_count, ..
                    } => {
                        register_used_in_address(register, base)
                            || register_used_in_value(register, item_count)
                    }
                    OpcodeKind::Read(a) => register_used_in_address(register, a),
                    OpcodeKind::ReadBits(a, _, _) => register_used_in_address(register, a),
                    OpcodeKind::ReadOffset(a, _) => register_used_in_address(register, a),
                    OpcodeKind::WriteBits(a, r, _, _) => {
                        register_used_in_address(register, a) || r == register
                    }
                    OpcodeKind::WriteOffset(a, v, _) => {
                        register_used_in_address(register, a) || register_used_in_value(register, v)
                    }
                }
            }
        })
        .collect()
}

fn register_used_in_value(register: Register, value: Value) -> bool {
    match value {
        Value::Register(r) => r == register,
        Value::Immediate(_) => false,
        Value::Indirect(a) => register_used_in_address(register, a),
    }
}

fn register_used_in_address(register: Register, addr: Address) -> bool {
    match addr {
        Address::Register(r) => r == register,
        Address::Immediate(_) => false,
    }
}
