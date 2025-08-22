pub mod blocks;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use crate::{
    analyser::{
        Circ, CircId, Connection, ConnectionEndpoint, ConnectionRange, LibraryCirc, ModuleCirc,
        Pin, PinId,
    },
    ast::PinDirection,
    optimiser::{BitEndpoint, OptimiserUnit, TruthTable},
    util::{div_up, invert_hashmap},
};
pub use blocks::{Block, BlockAddress, Sections};

pub trait WordTrait:
    Copy + Default + Eq + std::fmt::Display + std::ops::Add + std::ops::Mul + std::hash::Hash
{
    fn offset(&self, disp: usize, scale: usize) -> usize;
}

pub type Register = usize;

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
pub enum Constant<Word: WordTrait> {
    Value(Word),
    Address(BlockAddress),
}

impl<Word: WordTrait> std::fmt::Display for Constant<Word> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(v) => write!(f, "{v}"),
            Self::Address((blk, addr)) => write!(f, "b{blk}:{addr}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Value<Word: WordTrait> {
    Register(Register),
    Immediate(Constant<Word>),
    Indirect(Address),
}

impl<Word: WordTrait> std::fmt::Display for Value<Word> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Register(n) => write!(f, "r{n}"),
            Self::Immediate(i) => write!(f, "{i}"),
            Self::Indirect(addr) => write!(f, "[{addr}]"),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum OpcodeKind<Word: WordTrait> {
    Call(Address, Value<Word>),
    CallLibrary(usize, Register, Register, Register),
    CheckInputs(Register, usize, usize),
    Load(Constant<Word>),
    LoadResv,
    Offset {
        base: Address,
        item_size: usize,
        item_count: Value<Word>,
    },
    Read(Address),
    ReadBits(Address, usize, usize),
    ReadOffset(Address, usize),
    WriteBits(Address, Register, usize, usize),
    WriteOffset(Address, Value<Word>, usize),
}

impl<Word: WordTrait> std::fmt::Display for OpcodeKind<Word> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call(a, v) => write!(f, "call {a}({v})"),
            Self::CallLibrary(c, r, i, o) => write!(f, "call ft[{c}](r{r}, r{i}, r{o})"),
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

struct CircResvBlock<Word>(std::marker::PhantomData<Word>);

impl<Word> CircResvBlock<Word> {
    fn get_input_offset(circ: &Circ) -> usize {
        match circ {
            Circ::ModuleCirc(_) => ModuleCircResvBlock::<Word>::get_input_offset(),
            Circ::LibraryCirc(_) => LibraryCircResvBlock::<Word>::get_input_offset(),
        }
    }

    fn get_input_size(circ: &Circ) -> usize {
        match circ {
            Circ::ModuleCirc(c) => ModuleCircResvBlock::<Word>::get_input_size(c),
            Circ::LibraryCirc(c) => LibraryCircResvBlock::<Word>::get_input_size(c),
        }
    }

    fn get_old_input_offset(circ: &Circ) -> usize {
        match circ {
            Circ::ModuleCirc(c) => ModuleCircResvBlock::<Word>::get_old_input_offset(c),
            Circ::LibraryCirc(c) => LibraryCircResvBlock::<Word>::get_old_input_offset(c),
        }
    }

    fn get_output_offset(circ: &Circ) -> usize {
        match circ {
            Circ::ModuleCirc(c) => ModuleCircResvBlock::<Word>::get_output_offset(c),
            Circ::LibraryCirc(c) => LibraryCircResvBlock::<Word>::get_output_offset(c),
        }
    }

    fn get_output_size(circ: &Circ) -> usize {
        match circ {
            Circ::ModuleCirc(c) => ModuleCircResvBlock::<Word>::get_output_size(c),
            Circ::LibraryCirc(c) => LibraryCircResvBlock::<Word>::get_output_size(c),
        }
    }
}

struct ModuleCircResvBlock<Word> {
    inputs: usize,
    inputs_size: usize,
    inputs_old: usize,
    inputs_old_size: usize,
    outputs: usize,
    outputs_size: usize,
    dependency_ptrs: usize,
    dependency_ptrs_size: usize,
    _phantom: std::marker::PhantomData<Word>,
}

impl<Word> ModuleCircResvBlock<Word> {
    const WORD_BITS: usize = std::mem::size_of::<Word>() * 8;

    fn new(circ: &ModuleCirc) -> Self {
        Self {
            inputs: Self::get_input_offset(),
            inputs_size: Self::get_input_size(circ),
            inputs_old: Self::get_old_input_offset(circ),
            inputs_old_size: Self::get_input_size(circ),
            outputs: Self::get_output_offset(circ),
            outputs_size: Self::get_output_size(circ),
            dependency_ptrs: Self::get_dependency_ptrs_offset(circ),
            dependency_ptrs_size: circ.dependencies.len(),
            _phantom: std::marker::PhantomData,
        }
    }

    fn get_input_offset() -> usize {
        0
    }

    fn get_input_size(circ: &ModuleCirc) -> usize {
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
            Self::WORD_BITS,
        )
    }

    fn get_old_input_offset(circ: &ModuleCirc) -> usize {
        Self::get_input_offset() + Self::get_input_size(circ)
    }

    fn get_output_offset(circ: &ModuleCirc) -> usize {
        Self::get_old_input_offset(circ) + Self::get_input_size(circ)
    }

    fn get_output_size(circ: &ModuleCirc) -> usize {
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
            Self::WORD_BITS,
        )
    }

    fn get_dependency_ptrs_offset(circ: &ModuleCirc) -> usize {
        Self::get_output_offset(circ) + Self::get_output_size(circ)
    }
}

struct LibraryCircResvBlock<Word> {
    inputs: usize,
    inputs_size: usize,
    inputs_old: usize,
    inputs_old_size: usize,
    outputs: usize,
    outputs_size: usize,
    resv: usize,
    resv_size: usize,
    _phantom: std::marker::PhantomData<Word>,
}

impl<Word> LibraryCircResvBlock<Word> {
    const WORD_BITS: usize = std::mem::size_of::<Word>() * 8;

    fn new(circ: &LibraryCirc) -> Self {
        Self {
            inputs: Self::get_input_offset(),
            inputs_size: Self::get_input_size(circ),
            inputs_old: Self::get_old_input_offset(circ),
            inputs_old_size: Self::get_input_size(circ),
            outputs: Self::get_output_offset(circ),
            outputs_size: Self::get_output_size(circ),
            resv: Self::get_resv_offset(circ),
            resv_size: circ.mem_size,
            _phantom: std::marker::PhantomData,
        }
    }

    fn get_input_offset() -> usize {
        0
    }

    fn get_input_size(circ: &LibraryCirc) -> usize {
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
            Self::WORD_BITS,
        )
    }

    fn get_old_input_offset(circ: &LibraryCirc) -> usize {
        Self::get_input_offset() + Self::get_input_size(circ)
    }

    fn get_output_offset(circ: &LibraryCirc) -> usize {
        Self::get_old_input_offset(circ) + Self::get_input_size(circ)
    }

    fn get_output_size(circ: &LibraryCirc) -> usize {
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
            Self::WORD_BITS,
        )
    }

    fn get_resv_offset(circ: &LibraryCirc) -> usize {
        Self::get_output_size(circ) + Self::get_output_offset(circ)
    }
}

struct TtResvBlock<Word> {
    inputs: usize,
    inputs_size: usize,
    inputs_old: usize,
    inputs_old_size: usize,
    outputs: usize,
    outputs_size: usize,
    _phantom: std::marker::PhantomData<Word>,
}

impl<Word> TtResvBlock<Word> {
    const WORD_BITS: usize = std::mem::size_of::<Word>() * 8;

    fn new(tt: &TruthTable) -> Self {
        Self {
            inputs: Self::get_input_offset(),
            inputs_size: Self::get_input_size(tt),
            inputs_old: Self::get_old_input_offset(tt),
            inputs_old_size: Self::get_input_size(tt),
            outputs: Self::get_output_offset(tt),
            outputs_size: Self::get_output_size(tt),
            _phantom: std::marker::PhantomData,
        }
    }

    fn get_input_offset() -> usize {
        0
    }

    fn get_input_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.inputs.len(), Self::WORD_BITS)
    }

    fn get_old_input_offset(tt: &TruthTable) -> usize {
        Self::get_input_offset() + Self::get_input_size(tt)
    }

    fn get_output_offset(tt: &TruthTable) -> usize {
        Self::get_old_input_offset(tt) + Self::get_input_size(tt)
    }

    fn get_output_size(tt: &TruthTable) -> usize {
        div_up(2 * tt.outputs.len(), Self::WORD_BITS)
    }
}

#[derive(Debug)]
struct CodeBlockBuilder<Word: WordTrait> {
    register_allocator: RegisterAllocator,
    opcodes: Vec<Opcode<Word>>,
}

impl<Word: WordTrait> Default for CodeBlockBuilder<Word> {
    fn default() -> Self {
        Self {
            register_allocator: RegisterAllocator::default(),
            opcodes: Vec::new(),
        }
    }
}

#[allow(unused)]
impl<Word: WordTrait> CodeBlockBuilder<Word> {
    fn new() -> Self {
        Self::default()
    }

    fn load(&mut self, value: Constant<Word>) -> Register {
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

    fn offset(
        &mut self,
        base_addr: Address,
        item_size: usize,
        item_count: Value<Word>,
    ) -> Register {
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
        item_count: Value<Word>,
    ) -> Register {
        self.offset(Address::Immediate(address), item_size, item_count)
    }

    fn offset_reg(
        &mut self,
        base_register: Register,
        item_size: usize,
        item_count: Value<Word>,
    ) -> Register {
        self.offset(Address::Register(base_register), item_size, item_count)
    }

    fn offset_reg_imm(
        &mut self,
        base_register: Register,
        item_size: usize,
        item_count: Word,
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

    fn call(&mut self, addr: Address, arg: Value<Word>) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::Call(addr, arg),
        });
    }

    fn call_library(
        &mut self,
        circ_id: usize,
        resv_ptr: Register,
        inputs_addr: Register,
        outputs_addr: Register,
    ) {
        let destination = self.register_allocator.next();
        self.opcodes.push(Opcode {
            destination,
            opcode: OpcodeKind::CallLibrary(circ_id, resv_ptr, inputs_addr, outputs_addr),
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

    fn write_offset(&mut self, addr: Address, data: Value<Word>, offset: usize) {
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
pub struct Opcode<Word: WordTrait> {
    pub destination: Register,
    pub opcode: OpcodeKind<Word>,
}

impl<Word: WordTrait> std::fmt::Display for Opcode<Word> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{} = {}", self.destination, self.opcode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CircEndpoint {
    Pin(PinId),
    Dependency(usize, PinId),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DataItem<Word: WordTrait> {
    Address(BlockAddress),
    Word(Word),
}

impl<Word: WordTrait> Default for DataItem<Word> {
    fn default() -> Self {
        Self::Word(Word::default())
    }
}

impl<Word: WordTrait> std::fmt::Display for DataItem<Word> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Address((blk, addr)) => write!(f, "b{blk}:{addr}"),
            Self::Word(word) => write!(f, "{word}"),
        }
    }
}

#[derive(Debug)]
pub struct IrGen<'a, Word: WordTrait> {
    units: &'a [OptimiserUnit],
    external_circ_count: usize,
    external_libraries: Vec<Rc<Path>>,
    sections: Sections<Word>,
    code_blocks: HashMap<usize, usize>,
}

impl<'a, Word: WordTrait> IrGen<'a, Word>
where
    Word: Clone + Copy + Default,
{
    pub fn new(
        units: &'a [OptimiserUnit],
        external_circ_count: usize,
        external_libraries: Vec<Rc<Path>>,
    ) -> Self {
        Self {
            units,
            external_circ_count,
            external_libraries,
            sections: Sections::default(),
            code_blocks: HashMap::new(),
        }
    }

    fn get_level(
        &self,
        connections_grouped_by_source: &HashMap<CircEndpoint, Vec<Connection>>,
        endpoint_levels: &mut HashMap<CircEndpoint, usize>,
        connection_levels: &mut HashMap<Connection, usize>,
        dependency_levels: &mut HashMap<usize, usize>,
        circ: &ModuleCirc,
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
                    c.pins()
                        .value_indices()
                        .filter_map(|(i, p)| {
                            (p.direction == PinDirection::Input)
                                .then_some(CircEndpoint::Dependency(dependency, i))
                        })
                        .collect::<HashSet<CircEndpoint>>(),
                    c.pins()
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
                .flat_map(|endpoint| connections_grouped_by_source[endpoint].iter());
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
        circ: &ModuleCirc,
    ) -> (HashMap<Connection, usize>, HashMap<usize, usize>) {
        let mut connection_levels = circ
            .pins
            .value_indices()
            .filter_map(|(id, p)| {
                (p.direction == PinDirection::Input).then_some(CircEndpoint::Pin(id))
            })
            .flat_map(|endpoint| connections_grouped_by_source[&endpoint].iter().copied())
            .zip(std::iter::repeat(0))
            .collect::<HashMap<_, _>>();

        let mut dependency_levels = circ
            .dependencies
            .iter()
            .enumerate()
            .filter_map(|(dep_id, circ_id)| {
                match &self.units[*circ_id] {
                    OptimiserUnit::Circ(c) => c
                        .pins()
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

        connection_levels.extend(
            dependency_levels
                .keys()
                .map(|&d| (d, circ.dependencies[d]))
                .filter_map(|(d, unit_id)| match &self.units[unit_id] {
                    OptimiserUnit::Circ(c) => Some((d, c)),
                    _ => None,
                })
                .flat_map(|(d, c)| {
                    c.pins().value_indices().filter_map(move |(i, p)| {
                        (p.direction == PinDirection::Output)
                            .then_some(CircEndpoint::Dependency(d, i))
                    })
                })
                .flat_map(|endpoint| connections_grouped_by_source[&endpoint].iter().copied())
                .zip(std::iter::repeat(0)),
        );

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

    fn emit_lib_strtab(&mut self) -> usize {
        let strtab = self.sections.new_strtab();

        for path in self.external_libraries.iter() {
            let path = path.display().to_string();
            strtab.push(path.into_boxed_str());
        }

        strtab.id
    }

    fn emit_tick_table(&mut self) -> usize {
        let tick_table = self.sections.new_data();

        tick_table.reserve(self.external_circ_count);

        tick_table.id
    }
}

macro_rules! word_size_impl {
    ($t:ty) => {
        impl WordTrait for $t {
            fn offset(&self, disp: usize, scale: usize) -> usize {
                (disp + scale * *self as usize)
            }
        }

        impl<'a> IrGen<'a, $t> {
            const WORD_SIZE: usize = std::mem::size_of::<$t>();
            const WORD_BITS: usize = <$t>::BITS as usize;

            pub fn emit(mut self, main: CircId) -> Sections<$t> {
                let OptimiserUnit::Circ(circ) = &self.units[main] else {
                    unreachable!();
                };

                let lib_strtab_id = self.emit_lib_strtab();
                self.sections.label("_lib_strtab", (lib_strtab_id, 0));

                let tick_table_id = self.emit_tick_table();
                self.sections.label("_tick_table", (tick_table_id, 0));

                let (main_code_block_id, main_resv_block_id) = self.emit_circ_blocks(main, circ);

                self.sections.label("_main", (main_code_block_id, 0));
                self.sections.label("_main_data", (main_resv_block_id, 0));

                self.sections
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
                match circ {
                    Circ::ModuleCirc(circ) => self.emit_module_circ_resv_block(circ),
                    Circ::LibraryCirc(circ) => self.emit_library_circ_resv_block(circ),
                }
            }

            fn emit_module_circ_resv_block(&mut self, circ: &ModuleCirc) -> usize {
                let offsets = ModuleCircResvBlock::<$t>::new(circ);

                let dependency_resv_blocks = circ.dependencies.iter().map(|&unit_id|
                            match &self.units[unit_id] {
                                OptimiserUnit::Circ(circ) => self.emit_circ_blocks(unit_id, circ),
                                OptimiserUnit::TruthTable(tt) => self.emit_tt_blocks(unit_id, tt),
                            }.1
                        ).collect::<Vec<_>>();

                let circ_resv = self.sections.new_data();

                assert_eq!(circ_resv.offset(), offsets.inputs);
                circ_resv.reserve(offsets.inputs_size);

                assert_eq!(circ_resv.offset(), offsets.inputs_old);
                circ_resv.reserve(offsets.inputs_old_size);

                assert_eq!(circ_resv.offset(), offsets.outputs);
                circ_resv.reserve(offsets.outputs_size);

                assert_eq!(circ_resv.offset(), offsets.dependency_ptrs);

                for block_id in dependency_resv_blocks {
                    circ_resv.push(DataItem::Address((block_id, 0)));
                }

                circ_resv.id
            }

            fn emit_library_circ_resv_block(&mut self, circ: &LibraryCirc) -> usize {
                let offsets = LibraryCircResvBlock::<$t>::new(circ);

                let circ_resv = self.sections.new_data();

                assert_eq!(circ_resv.offset(), offsets.inputs);
                circ_resv.reserve(offsets.inputs_size);

                assert_eq!(circ_resv.offset(), offsets.inputs_old);
                circ_resv.reserve(offsets.inputs_old_size);

                assert_eq!(circ_resv.offset(), offsets.outputs);
                circ_resv.reserve(offsets.outputs_size);

                assert_eq!(circ_resv.offset(), offsets.resv);

                let mut resv =
                    vec![<$t>::default(); div_up(circ.mem_size, std::mem::size_of::<$t>())];

                let args = circ.signature.get_library_circ_args();
                unsafe {
                    (circ.initialise)(resv.as_mut_ptr() as *mut (), args.len(), args.as_ptr());
                };

                let resv = resv.into_iter().map(DataItem::Word).collect::<Vec<_>>();
                circ_resv.extend(&resv);

                circ_resv.id
            }

            fn emit_module_circ_code_block(&mut self, unit_id: usize, circ: &ModuleCirc) -> usize {
                let connections_grouped_by_source = circ
                    .connections
                    .iter()
                    .map(|conn| match conn.source {
                        ConnectionEndpoint::Pin { pin_id, .. } => {
                            (CircEndpoint::Pin(pin_id), *conn)
                        }
                        ConnectionEndpoint::Dependency {
                            dependency_id,
                            pin_id,
                            ..
                        } => (CircEndpoint::Dependency(dependency_id, pin_id), *conn),
                    })
                    .fold(
                        HashMap::new(),
                        |mut hm, (k, v)| -> HashMap<CircEndpoint, Vec<Connection>> {
                            hm.entry(k).or_default().push(v);
                            hm
                        },
                    );

                let (connection_levels, dependency_levels) =
                    self.levelise_connections(connections_grouped_by_source, circ);

                if (0..circ.dependencies.len()).any(|d| !dependency_levels.contains_key(&d)) {
                    dbg!(&circ);
                    dbg!(&dependency_levels);
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

                let offsets = ModuleCircResvBlock::<$t>::new(circ);

                let mut code_block = CodeBlockBuilder::<$t>::new();

                let resv_ptr = code_block.load_resv();

                code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);

                let dependency_ptrs =
                    code_block.offset_reg_imm(resv_ptr, 1, offsets.dependency_ptrs as $t);

                for (dependencies_to_update, connections_to_process) in processing_order {
                    for dependency_id in dependencies_to_update {
                        let dep_resv = code_block.offset_reg_imm(dependency_ptrs, 1, dependency_id as $t);
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

                        let (dest_base, mut dest_word, dest_start, dest_end) = match connection.dest
                        {
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

                        const WORD_BITS: usize = <$t>::BITS as usize / 2;

                        let mut src_index = src_start;
                        let mut dest_index = dest_start;
                        loop {
                            let bits_to_write = std::cmp::min(
                                std::cmp::min(
                                    WORD_BITS - (src_index % WORD_BITS),
                                    src_end - src_index,
                                ),
                                std::cmp::min(
                                    WORD_BITS - (dest_index % WORD_BITS),
                                    dest_end - dest_index,
                                ),
                            );

                            let r_src_word = code_block.offset_reg_imm(src_base, 1, src_word as $t);
                            let r_dest_word = code_block.offset_reg_imm(dest_base, 1, dest_word as $t);

                            let r0 = code_block.read_bits_reg(
                                r_src_word,
                                src_index,
                                src_index + bits_to_write,
                            );
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

                let circ_code = self.sections.new_text();
                circ_code.extend(&code_block.opcodes);

                circ_code.id
            }

            fn get_pin_offset(&self, unit_id: usize, pin_id: PinId) -> (usize, usize) {
                let unit = &self.units[unit_id];
                match unit {
                    OptimiserUnit::Circ(circ) => {
                        let pin = circ.pins().get_by_index(pin_id).unwrap();
                        match pin {
                            Pin {
                                direction: PinDirection::Input,
                                ..
                            } => {
                                let pin_starts = circ
                                    .pins()
                                    .value_indices()
                                    .filter(|(_, p)| p.direction == PinDirection::Input)
                                    .map(|(id, p)| std::iter::repeat_n(id, p.width).enumerate())
                                    .flatten()
                                    .enumerate()
                                    .filter_map(|(start, (i, pin_id))| {
                                        (i == 0).then_some((pin_id, start))
                                    })
                                    .collect::<HashMap<PinId, usize>>();

                                let bit_offset = 2 * pin_starts[&pin_id];
                                let word_offset = bit_offset / <$t>::BITS as usize;
                                let bit_offset = (bit_offset % <$t>::BITS as usize) / 2;

                                (
                                    CircResvBlock::<$t>::get_input_offset(circ) + word_offset,
                                    bit_offset,
                                )
                            }
                            Pin {
                                direction: PinDirection::Output,
                                ..
                            } => {
                                let pin_starts = circ
                                    .pins()
                                    .value_indices()
                                    .filter(|(_, p)| p.direction == PinDirection::Output)
                                    .map(|(id, p)| std::iter::repeat_n(id, p.width).enumerate())
                                    .flatten()
                                    .enumerate()
                                    .filter_map(|(start, (i, pin_id))| {
                                        (i == 0).then_some((pin_id, start))
                                    })
                                    .collect::<HashMap<PinId, usize>>();

                                let bit_offset = 2 * pin_starts[&pin_id];
                                let word_offset = bit_offset / <$t>::BITS as usize;
                                let bit_offset = (bit_offset % <$t>::BITS as usize) / 2;

                                (
                                    CircResvBlock::<$t>::get_output_offset(circ) + word_offset,
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
                                    BitEndpoint::Pin { pin_id: p, .. } => {
                                        (*p == pin_id).then_some(s)
                                    }
                                    _ => unreachable!(),
                                })
                                .zip(std::iter::repeat(PinDirection::Output))
                                .min_by_key(|(x, _)| **x))
                            .unwrap();

                        let bit_offset = 2 * bit_offset;
                        let word_offset = bit_offset / <$t>::BITS as usize;
                        let bit_offset = (bit_offset % <$t>::BITS as usize) / 2;

                        (
                            match direction {
                                PinDirection::Input => TtResvBlock::<$t>::get_input_offset(),
                                PinDirection::Output => TtResvBlock::<$t>::get_output_offset(tt),
                                _ => unreachable!(),
                            } + word_offset,
                            bit_offset,
                        )
                    }
                }
            }

            fn emit_circ_code_block(&mut self, unit_id: usize, circ: &Circ) -> usize {
                match circ {
                    Circ::ModuleCirc(circ) => self.emit_module_circ_code_block(unit_id, circ),
                    Circ::LibraryCirc(circ) => self.emit_library_circ_code_block(circ),
                }
            }

            fn emit_library_circ_code_block(&mut self, circ: &LibraryCirc) -> usize {
                let offsets = LibraryCircResvBlock::<$t>::new(circ);

                let mut code_block = CodeBlockBuilder::<$t>::new();

                let resv_ptr = code_block.load_resv();

                // if circ.tick_behaviour == TickBehaviour::OnChange {
                //     code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);
                // }

                code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);

                let inputs_addr = if offsets.inputs_size == 0 {
                     code_block.load(Constant::Value(0 as $t))
                } else {
                     code_block.offset_reg_imm(resv_ptr, 1, offsets.inputs as $t)
                };

                let outputs_addr = if offsets.outputs_size == 0 {
                     code_block.load(Constant::Value(0 as $t))
                } else {
                     code_block.offset_reg_imm(resv_ptr, 1, offsets.outputs as $t)
                };

                let resv = code_block.offset_reg_imm(resv_ptr, 1, offsets.resv as $t);

                code_block.call_library(circ.circ_index, resv, inputs_addr, outputs_addr);

                let circ_code = self.sections.new_text();
                circ_code.extend(&code_block.opcodes);

                circ_code.id
            }

            fn emit_tt_data_block(&mut self, tt: &TruthTable) -> usize {
                let offsets = TtResvBlock::<$t>::new(tt);

                let tt_data = self.sections.new_rodata();

                for row in tt.iter_rows() {
                    let bytes = row.value().to_ne_bytes();
                    for i in 0..offsets.outputs_size {
                        let index = i * Self::WORD_SIZE;
                        let mut byte_array = [0u8; Self::WORD_SIZE];
                        byte_array.copy_from_slice(&bytes[index..index + Self::WORD_SIZE]);
                        tt_data.push(<$t>::from_ne_bytes(byte_array));
                    }
                }

                tt_data.id
            }

            fn emit_tt_resv_block(&mut self, tt: &TruthTable) -> usize {
                let offsets = TtResvBlock::<$t>::new(tt);

                let tt_resv = self.sections.new_data();

                assert!(tt_resv.offset() == offsets.inputs);
                tt_resv.reserve(offsets.inputs_size);

                assert!(tt_resv.offset() == offsets.inputs_old);
                tt_resv.reserve(offsets.inputs_old_size);

                assert!(tt_resv.offset() == offsets.outputs);
                tt_resv.reserve(offsets.outputs_size);

                tt_resv.id
            }

            fn emit_tt_code_block(&mut self, tt: &TruthTable, tt_data_block: usize) -> usize {
                let offsets = TtResvBlock::<$t>::new(tt);

                let mut code_block = CodeBlockBuilder::<$t>::new();

                let resv_ptr = code_block.load_resv();
                code_block.check_inputs(resv_ptr, offsets.inputs, offsets.inputs_old);

                assert_eq!(offsets.inputs_size, 1);

                let r0 = code_block.read_offset(Address::Register(resv_ptr), offsets.inputs);

                let data_ptr = self.sections.get_rodata(tt_data_block).base_addr();
                let r1 = code_block.offset(
                    Address::Immediate(data_ptr),
                    offsets.outputs_size,
                    Value::Register(r0),
                );

                let outputs_addr = code_block.offset_reg_imm(resv_ptr, 1, offsets.outputs as $t);

                for i in 0..offsets.outputs_size {
                    let r2 = code_block.read_offset(Address::Register(r1), i);
                    code_block.write_offset(Address::Register(outputs_addr), Value::Register(r2), i);
                }

                let tt_code = self.sections.new_text();
                tt_code.extend(&code_block.opcodes);

                tt_code.id
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
    };
}

word_size_impl!(u16);
word_size_impl!(u32);
word_size_impl!(u64);

pub fn get_register_usage<Word: WordTrait>(block: &Block<Opcode<Word>>) -> Vec<Vec<bool>> {
    block
        .iter()
        .map(|opcode| register_is_used(block, opcode.destination))
        .collect()
}

fn register_is_used<Word: WordTrait>(block: &Block<Opcode<Word>>, register: Register) -> Vec<bool> {
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
                    OpcodeKind::CallLibrary(_, r, i, o) => {
                        r == register || i == register || o == register
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

fn register_used_in_value<Word: WordTrait>(register: Register, value: Value<Word>) -> bool {
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

pub fn generate_16(
    units: &[OptimiserUnit],
    main: CircId,
    external_circ_count: usize,
    external_libraries: Vec<Rc<Path>>,
) -> Sections<u16> {
    IrGen::<u16>::new(units, external_circ_count, external_libraries).emit(main)
}

pub fn generate_32(
    units: &[OptimiserUnit],
    main: CircId,
    external_circ_count: usize,
    external_libraries: Vec<Rc<Path>>,
) -> Sections<u32> {
    IrGen::<u32>::new(units, external_circ_count, external_libraries).emit(main)
}

pub fn generate_64(
    units: &[OptimiserUnit],
    main: CircId,
    external_circ_count: usize,
    external_libraries: Vec<Rc<Path>>,
) -> Sections<u64> {
    IrGen::<u64>::new(units, external_circ_count, external_libraries).emit(main)
}
