use crate::codegen::blocks::{Block, BlockAddress};
use crate::codegen::targets::ir::{
    Address, Constant, IrBlocks, Opcode, OpcodeKind, Register, Value,
};

use std::collections::HashMap;

pub struct IrOptimiser {
    blocks: IrBlocks,
    block_id_mapping: HashMap<usize, usize>,
}

impl IrOptimiser {
    pub fn optimise(blocks: IrBlocks) -> IrBlocks {
        let mut optimiser = Self {
            blocks: IrBlocks::default(),
            block_id_mapping: HashMap::new(),
        };

        optimiser.blocks.set_entry(blocks.get_entry());

        optimiser.deduplicate_data_blocks(&blocks);
        optimiser.optimise_code_blocks(&blocks);

        optimiser.remap_blocks();

        optimiser.blocks
    }

    fn optimise_code_blocks(&mut self, blocks: &IrBlocks) {
        let mut deduped_blocks = HashMap::new();

        for block in blocks.code_blocks() {
            let new_opcodes = CodeBlockOptimser::optimise(block);
            if let Some(block_id) = deduped_blocks.get(&new_opcodes) {
                self.block_id_mapping.insert(block.id, *block_id);
            } else {
                let new_block = self.blocks.new_code();
                new_block.extend(&new_opcodes);
                deduped_blocks.insert(new_opcodes, new_block.id);
                self.block_id_mapping.insert(block.id, new_block.id);
            }
        }
    }

    fn deduplicate_data_blocks(&mut self, blocks: &IrBlocks) {
        let mut duplicate_blocks = HashMap::new();

        for block in blocks.data_blocks() {
            let items = block.items();
            if let Some(block_id) = duplicate_blocks.get(items) {
                self.block_id_mapping.insert(block.id, *block_id);
            } else {
                let new_block = self.blocks.new_data();
                new_block.extend(items);
                duplicate_blocks.insert(new_block.items().to_vec(), new_block.id);
                self.block_id_mapping.insert(block.id, new_block.id);
            }
        }
    }

    fn remap_blocks(&mut self) {
        for code_block in self.blocks.code_blocks_mut() {
            code_block.map_items(|opc| Opcode {
                destination: opc.destination,
                opcode: CodeBlockOptimser::remap_blocks(&self.block_id_mapping, opc.opcode),
            })
        }

        let (entry_block_id, addr) = self.blocks.get_entry();
        self.blocks
            .set_entry((self.block_id_mapping[&entry_block_id], addr));
    }
}

struct CodeBlockOptimser {
    duplicate_instructions: HashMap<OpcodeKind, Register>,
    constant_registers: HashMap<Register, Constant>,
    duplicate_registers: HashMap<Register, Register>,
}

impl CodeBlockOptimser {
    fn optimise(source: &Block<Opcode>) -> Vec<Opcode> {
        let mut optimiser = Self {
            duplicate_instructions: HashMap::new(),
            constant_registers: HashMap::new(),
            duplicate_registers: HashMap::new(),
        };

        let mut register_mapping = HashMap::new();
        let new_instructions = source
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, opc)| {
                match optimiser.simplify_opcode(opc.destination, opc.opcode) {
                    Some(opc) => Some((index, opc)),
                    None => None,
                }
            })
            .enumerate()
            .map(|(new_index, (old_index, opc))| {
                register_mapping.insert(old_index, new_index);
                Opcode {
                    destination: new_index,
                    opcode: Self::update_register_numbers(&register_mapping, opc),
                }
            })
            .collect::<Vec<Opcode>>();

        // TODO: reorder opcodes to only calculcate registers just before they are needed

        new_instructions
    }

    fn simplify_opcode(&mut self, destination: Register, opcode: OpcodeKind) -> Option<OpcodeKind> {
        let new_op = match opcode {
            OpcodeKind::Add(reg, value) => self.simplify_add(destination, reg, value)?,
            OpcodeKind::Call(addr) => OpcodeKind::Call(self.simplify_address(addr)),
            OpcodeKind::Load(value) => self.simplify_load(destination, value)?,
            OpcodeKind::Mul(reg, value) => self.simplify_mul(destination, reg, value)?,
            OpcodeKind::Pop => OpcodeKind::Pop,
            OpcodeKind::Push(value) => OpcodeKind::Push(self.simplify_value(value)),
            OpcodeKind::Read(addr) => OpcodeKind::Read(self.simplify_address(addr)),
            OpcodeKind::ReadBits(addr, s, e) => {
                OpcodeKind::ReadBits(self.simplify_address(addr), s, e)
            }
            OpcodeKind::ReadOffset(addr, offset) => {
                OpcodeKind::ReadOffset(self.simplify_address(addr), self.simplify_value(offset))
            }
            OpcodeKind::Return => OpcodeKind::Return,
            OpcodeKind::Write(addr, value) => {
                OpcodeKind::Write(self.simplify_address(addr), self.simplify_value(value))
            }
            OpcodeKind::WriteBits(addr, reg, s, e) => OpcodeKind::WriteBits(
                self.simplify_address(addr),
                self.simplify_register(reg),
                s,
                e,
            ),
            OpcodeKind::WriteOffset(addr, reg, offset) => OpcodeKind::WriteOffset(
                self.simplify_address(addr),
                self.simplify_register(reg),
                self.simplify_value(offset),
            ),
        };

        let duplicate_of_reg = match new_op {
            OpcodeKind::Add(reg, Value::Immediate(Constant::Value(0))) => Some(reg),
            OpcodeKind::Mul(_, Value::Immediate(Constant::Value(0))) => {
                self.constant_registers
                    .insert(destination, Constant::Value(0));
                return None;
            }
            OpcodeKind::Mul(reg, Value::Immediate(Constant::Value(1))) => Some(reg),
            _ => None,
        };

        if let Some(reg) = duplicate_of_reg {
            self.duplicate_registers.insert(destination, reg);
            return None;
        };

        let can_be_eliminated = match new_op {
            OpcodeKind::Add(_, Value::Immediate(..) | Value::Register(..)) => true,
            OpcodeKind::Load(..) => true,
            OpcodeKind::Mul(_, Value::Immediate(..) | Value::Register(..)) => true,
            _ => false,
        };

        if can_be_eliminated {
            if let Some(reg) = self.duplicate_instructions.get(&new_op) {
                self.duplicate_registers.insert(destination, *reg);
                return None;
            } else {
                self.duplicate_instructions.insert(new_op, destination);
            }
        }

        Some(new_op)
    }

    fn simplify_add(
        &mut self,
        destination: Register,
        reg: Register,
        value: Value,
    ) -> Option<OpcodeKind> {
        if let Value::Immediate(v) = value {
            if let Value::Immediate(x) = self.simplify_register_maybe_value(reg) {
                let value = match (v, x) {
                    (Constant::Value(x), Constant::Value(y)) => Constant::Value(x + y),
                    (Constant::Address((xb, xa)), Constant::Address((yb, ya))) => {
                        assert_eq!(xb, yb);
                        Constant::Address((xb, xa + ya))
                    }
                    (Constant::Address((blk, x)), Constant::Value(y)) => {
                        Constant::Address((blk, x + y))
                    }
                    (Constant::Value(x), Constant::Address((blk, y))) => {
                        Constant::Address((blk, x + y))
                    }
                };

                self.constant_registers.insert(destination, value);
                return None;
            }
        }

        Some(OpcodeKind::Add(
            self.simplify_register(reg),
            self.simplify_value(value),
        ))
    }

    fn simplify_load(&mut self, destination: Register, value: Constant) -> Option<OpcodeKind> {
        self.constant_registers.insert(destination, value);
        // TODO: improve optimisation here so unused constant registers get removed
        Some(OpcodeKind::Load(value))
    }

    fn simplify_mul(
        &mut self,
        destination: Register,
        reg: Register,
        value: Value,
    ) -> Option<OpcodeKind> {
        if let Value::Immediate(v) = value {
            if let Value::Immediate(x) = self.simplify_register_maybe_value(reg) {
                let value = match (v, x) {
                    (Constant::Value(x), Constant::Value(y)) => Constant::Value(x * y),
                    (Constant::Address((xb, xa)), Constant::Address((yb, ya))) => {
                        assert_eq!(xb, yb);
                        Constant::Address((xb, xa * ya))
                    }
                    (Constant::Address((blk, x)), Constant::Value(y)) => {
                        Constant::Address((blk, x * y))
                    }
                    (Constant::Value(x), Constant::Address((blk, y))) => {
                        Constant::Address((blk, x * y))
                    }
                };

                self.constant_registers.insert(destination, value);
                return None;
            }
        }

        Some(OpcodeKind::Mul(
            self.simplify_register(reg),
            self.simplify_value(value),
        ))
    }

    fn simplify_register(&self, register: Register) -> Register {
        if let Some(&v) = self.duplicate_registers.get(&register) {
            self.simplify_register(v)
        } else {
            register
        }
    }

    fn simplify_register_maybe_value(&self, register: Register) -> Value {
        if let Some(&v) = self.constant_registers.get(&register) {
            Value::Immediate(v)
        } else if let Some(&v) = self.duplicate_registers.get(&register) {
            self.simplify_register_maybe_value(v)
        } else {
            Value::Register(register)
        }
    }

    fn simplify_address(&self, address: Address) -> Address {
        match address {
            Address::Register(r) => match self.simplify_register_maybe_value(r) {
                Value::Immediate(Constant::Address(a)) => Address::Immediate(a),
                _ => Address::Register(self.simplify_register(r)),
            },
            addr => addr,
        }
    }

    fn simplify_value(&self, value: Value) -> Value {
        match value {
            Value::Indirect(a) => Value::Indirect(self.simplify_address(a)),
            Value::Register(register) => self.simplify_register_maybe_value(register),
            value => value,
        }
    }

    fn update_register_numbers(m: &HashMap<usize, usize>, opcode: OpcodeKind) -> OpcodeKind {
        match opcode {
            OpcodeKind::Add(r, v) => {
                OpcodeKind::Add(m[&r], Self::update_register_number_in_value(m, v))
            }
            OpcodeKind::Call(a) => OpcodeKind::Call(Self::update_register_number_in_address(m, a)),
            OpcodeKind::Load(c) => OpcodeKind::Load(c),
            OpcodeKind::Mul(r, v) => {
                OpcodeKind::Mul(m[&r], Self::update_register_number_in_value(m, v))
            }
            OpcodeKind::Pop => OpcodeKind::Pop,
            OpcodeKind::Push(v) => OpcodeKind::Push(Self::update_register_number_in_value(m, v)),
            OpcodeKind::Read(a) => OpcodeKind::Read(Self::update_register_number_in_address(m, a)),
            OpcodeKind::ReadBits(a, s, e) => {
                OpcodeKind::ReadBits(Self::update_register_number_in_address(m, a), s, e)
            }
            OpcodeKind::ReadOffset(a, v) => OpcodeKind::ReadOffset(
                Self::update_register_number_in_address(m, a),
                Self::update_register_number_in_value(m, v),
            ),
            OpcodeKind::Return => OpcodeKind::Return,
            OpcodeKind::Write(a, v) => OpcodeKind::Write(
                Self::update_register_number_in_address(m, a),
                Self::update_register_number_in_value(m, v),
            ),
            OpcodeKind::WriteBits(a, r, s, e) => {
                OpcodeKind::WriteBits(Self::update_register_number_in_address(m, a), m[&r], s, e)
            }
            OpcodeKind::WriteOffset(a, r, v) => OpcodeKind::WriteOffset(
                Self::update_register_number_in_address(m, a),
                m[&r],
                Self::update_register_number_in_value(m, v),
            ),
        }
    }

    fn update_register_number_in_value(m: &HashMap<usize, usize>, value: Value) -> Value {
        match value {
            Value::Register(r) => Value::Register(m[&r]),
            Value::Indirect(Address::Register(r)) => Value::Indirect(Address::Register(m[&r])),
            value => value,
        }
    }

    fn update_register_number_in_address(m: &HashMap<usize, usize>, addr: Address) -> Address {
        match addr {
            Address::Register(r) => Address::Register(m[&r]),
            addr => addr,
        }
    }

    fn remap_blocks(m: &HashMap<usize, usize>, opcode: OpcodeKind) -> OpcodeKind {
        match opcode {
            OpcodeKind::Add(r, v) => OpcodeKind::Add(r, Self::remap_blocks_in_value(m, v)),
            OpcodeKind::Call(a) => OpcodeKind::Call(Self::remap_blocks_in_address(m, a)),
            OpcodeKind::Load(c) => OpcodeKind::Load(Self::remap_blocks_in_constant(m, c)),
            OpcodeKind::Mul(r, v) => OpcodeKind::Mul(r, Self::remap_blocks_in_value(m, v)),
            OpcodeKind::Pop => OpcodeKind::Pop,
            OpcodeKind::Push(v) => OpcodeKind::Push(Self::remap_blocks_in_value(m, v)),
            OpcodeKind::Read(a) => OpcodeKind::Read(Self::remap_blocks_in_address(m, a)),
            OpcodeKind::ReadBits(a, s, e) => {
                OpcodeKind::ReadBits(Self::remap_blocks_in_address(m, a), s, e)
            }
            OpcodeKind::ReadOffset(a, v) => OpcodeKind::ReadOffset(
                Self::remap_blocks_in_address(m, a),
                Self::remap_blocks_in_value(m, v),
            ),
            OpcodeKind::Return => OpcodeKind::Return,
            OpcodeKind::Write(a, v) => OpcodeKind::Write(
                Self::remap_blocks_in_address(m, a),
                Self::remap_blocks_in_value(m, v),
            ),
            OpcodeKind::WriteBits(a, r, s, e) => {
                OpcodeKind::WriteBits(Self::remap_blocks_in_address(m, a), r, s, e)
            }
            OpcodeKind::WriteOffset(a, r, v) => OpcodeKind::WriteOffset(
                Self::remap_blocks_in_address(m, a),
                r,
                Self::remap_blocks_in_value(m, v),
            ),
        }
    }

    fn remap_blocks_in_value(m: &HashMap<usize, usize>, value: Value) -> Value {
        match value {
            Value::Indirect(a) => Value::Indirect(Self::remap_blocks_in_address(m, a)),
            Value::Immediate(c) => Value::Immediate(Self::remap_blocks_in_constant(m, c)),
            value => value,
        }
    }

    fn remap_blocks_in_address(m: &HashMap<usize, usize>, address: Address) -> Address {
        match address {
            Address::Immediate((b, a)) => Address::Immediate((m[&b], a)),
            addr => addr,
        }
    }

    fn remap_blocks_in_constant(m: &HashMap<usize, usize>, value: Constant) -> Constant {
        match value {
            Constant::Address((b, a)) => Constant::Address((m[&b], a)),
            value => value,
        }
    }
}
