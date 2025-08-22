use crate::codegen::targets::ir::{
    blocks::Block, Address, Constant, DataItem, Opcode, OpcodeKind, Register, Sections, Value,
    WordTrait,
};

use std::collections::HashMap;

pub struct IrOptimiser<Word: WordTrait> {
    sections: Sections<Word>,
    block_id_mapping: HashMap<usize, usize>,
}

impl<Word: WordTrait> IrOptimiser<Word> {
    pub fn optimise(sections: Sections<Word>) -> Sections<Word> {
        let mut optimiser = Self {
            sections: Sections::default(),
            block_id_mapping: HashMap::new(),
        };

        for block in sections.data_blocks() {
            let new_id = optimiser.sections.push_data(block.items().to_vec());
            optimiser.block_id_mapping.insert(block.id, new_id);
        }

        for block in sections.strtab_blocks() {
            let new_id = optimiser.sections.push_strtab(block.items().to_vec());
            optimiser.block_id_mapping.insert(block.id, new_id);
        }

        optimiser.deduplicate_rodata_blocks(&sections);
        optimiser.optimise_code_blocks(&sections);

        optimiser.sections.labels = sections.labels;

        optimiser.remap_blocks();

        optimiser.sections
    }

    fn optimise_code_blocks(&mut self, blocks: &Sections<Word>) {
        let mut deduped_blocks = HashMap::new();

        for block in blocks.text_blocks() {
            let new_opcodes = CodeBlockOptimser::optimise(block);
            if let Some(block_id) = deduped_blocks.get(&new_opcodes) {
                self.block_id_mapping.insert(block.id, *block_id);
            } else {
                let new_block = self.sections.new_text();
                new_block.extend(&new_opcodes);
                deduped_blocks.insert(new_opcodes, new_block.id);
                self.block_id_mapping.insert(block.id, new_block.id);
            }
        }
    }

    fn deduplicate_rodata_blocks(&mut self, blocks: &Sections<Word>) {
        let mut duplicate_blocks = HashMap::new();

        for block in blocks.rodata_blocks() {
            let items = block.items();
            if let Some(block_id) = duplicate_blocks.get(items) {
                self.block_id_mapping.insert(block.id, *block_id);
            } else {
                let new_block = self.sections.new_rodata();
                new_block.extend(items);
                duplicate_blocks.insert(new_block.items().to_vec(), new_block.id);
                self.block_id_mapping.insert(block.id, new_block.id);
            }
        }
    }

    fn remap_blocks(&mut self) {
        for code_block in self.sections.text_blocks_mut() {
            code_block.map_items(|opc| Opcode {
                destination: opc.destination,
                opcode: CodeBlockOptimser::remap_blocks(&self.block_id_mapping, opc.opcode),
            })
        }

        for resv_block in self.sections.data_blocks_mut() {
            resv_block.map_items(|&c| match c {
                DataItem::Address((blk, addr)) => {
                    DataItem::Address((self.block_id_mapping[&blk], addr))
                }
                DataItem::Word(v) => DataItem::Word(v),
            });
        }

        self.sections.labels.values_mut().for_each(|addr| {
            let (blk, a) = addr;
            *addr = (self.block_id_mapping[blk], *a);
        });
    }
}

struct CodeBlockOptimser<Word: WordTrait> {
    duplicate_instructions: HashMap<OpcodeKind<Word>, Register>,
    constant_registers: HashMap<Register, Constant<Word>>,
    duplicate_registers: HashMap<Register, Register>,
}

impl<Word: WordTrait> CodeBlockOptimser<Word> {
    fn optimise(source: &Block<Opcode<Word>>) -> Vec<Opcode<Word>> {
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
            .collect::<Vec<Opcode<Word>>>();

        // TODO: reorder opcodes to only calculcate registers just before they are needed

        new_instructions
    }

    fn simplify_opcode(
        &mut self,
        destination: Register,
        opcode: OpcodeKind<Word>,
    ) -> Option<OpcodeKind<Word>> {
        let new_op = match opcode {
            OpcodeKind::Call(addr, value) => {
                OpcodeKind::Call(self.simplify_address(addr), self.simplify_value(value))
            }
            OpcodeKind::CallLibrary(c, r, i, o) => OpcodeKind::CallLibrary(
                c,
                self.simplify_register(r),
                self.simplify_register(i),
                self.simplify_register(o),
            ),
            OpcodeKind::CheckInputs(reg, i, o) => {
                OpcodeKind::CheckInputs(self.simplify_register(reg), i, o)
            }
            OpcodeKind::Load(value) => self.simplify_load(destination, value)?,
            OpcodeKind::LoadResv => OpcodeKind::LoadResv,
            OpcodeKind::Offset { .. } => self.simplify_offset(destination, opcode)?,
            OpcodeKind::Read(addr) => OpcodeKind::Read(self.simplify_address(addr)),
            OpcodeKind::ReadBits(addr, s, e) => {
                OpcodeKind::ReadBits(self.simplify_address(addr), s, e)
            }
            OpcodeKind::ReadOffset(addr, offset) => {
                OpcodeKind::ReadOffset(self.simplify_address(addr), offset)
            }
            OpcodeKind::WriteBits(addr, reg, s, e) => OpcodeKind::WriteBits(
                self.simplify_address(addr),
                self.simplify_register(reg),
                s,
                e,
            ),
            OpcodeKind::WriteOffset(addr, value, offset) => OpcodeKind::WriteOffset(
                self.simplify_address(addr),
                self.simplify_value(value),
                offset,
            ),
        };

        let duplicate_of_reg = match new_op {
            OpcodeKind::Offset {
                base: Address::Register(base_reg),
                item_size,
                item_count: Value::Immediate(Constant::Value(x)),
            } if x.offset(0, item_size) == 0 => Some(base_reg),
            _ => None,
        };

        if let Some(reg) = duplicate_of_reg {
            self.duplicate_registers.insert(destination, reg);
            return None;
        };

        let can_be_eliminated = match new_op {
            OpcodeKind::Load(..) => true,
            OpcodeKind::Offset { .. } => true,
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

    fn simplify_offset(
        &mut self,
        dest: Register,
        opcode: OpcodeKind<Word>,
    ) -> Option<OpcodeKind<Word>> {
        match opcode {
            OpcodeKind::Offset {
                base: Address::Immediate((blk, addr)),
                item_size,
                item_count: Value::Immediate(c),
            } => {
                let value = match c {
                    Constant::Address(_) => unreachable!(),
                    Constant::Value(v) => Constant::Address((blk, v.offset(addr, item_size))),
                };

                self.constant_registers.insert(dest, value);
                None
            }
            OpcodeKind::Offset {
                base,
                item_size,
                item_count,
            } => {
                let base = self.simplify_address(base);
                let item_count = self.simplify_value(item_count);
                Some(OpcodeKind::Offset {
                    base,
                    item_size,
                    item_count,
                })
            }
            _ => unreachable!(),
        }
    }

    fn simplify_load(
        &mut self,
        destination: Register,
        value: Constant<Word>,
    ) -> Option<OpcodeKind<Word>> {
        self.constant_registers.insert(destination, value);
        // TODO: improve optimisation here so unused constant registers get removed
        Some(OpcodeKind::Load(value))
    }

    fn simplify_register(&self, register: Register) -> Register {
        if let Some(&v) = self.duplicate_registers.get(&register) {
            self.simplify_register(v)
        } else {
            register
        }
    }

    fn simplify_register_maybe_value(&self, register: Register) -> Value<Word> {
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

    fn simplify_value(&self, value: Value<Word>) -> Value<Word> {
        match value {
            Value::Indirect(a) => Value::Indirect(self.simplify_address(a)),
            Value::Register(register) => self.simplify_register_maybe_value(register),
            value => value,
        }
    }

    fn update_register_numbers(
        m: &HashMap<usize, usize>,
        opcode: OpcodeKind<Word>,
    ) -> OpcodeKind<Word> {
        match opcode {
            OpcodeKind::Call(a, v) => OpcodeKind::Call(
                Self::update_register_number_in_address(m, a),
                Self::update_register_number_in_value(m, v),
            ),
            OpcodeKind::CallLibrary(c, r, i, o) => OpcodeKind::CallLibrary(c, m[&r], m[&i], m[&o]),
            OpcodeKind::CheckInputs(r, i, o) => OpcodeKind::CheckInputs(m[&r], i, o),
            OpcodeKind::Load(c) => OpcodeKind::Load(c),
            OpcodeKind::LoadResv => OpcodeKind::LoadResv,
            OpcodeKind::Offset {
                base,
                item_size,
                item_count,
            } => OpcodeKind::Offset {
                base: Self::update_register_number_in_address(m, base),
                item_size,
                item_count: Self::update_register_number_in_value(m, item_count),
            },
            OpcodeKind::Read(a) => OpcodeKind::Read(Self::update_register_number_in_address(m, a)),
            OpcodeKind::ReadBits(a, s, e) => {
                OpcodeKind::ReadBits(Self::update_register_number_in_address(m, a), s, e)
            }
            OpcodeKind::ReadOffset(a, v) => {
                OpcodeKind::ReadOffset(Self::update_register_number_in_address(m, a), v)
            }
            OpcodeKind::WriteBits(a, r, s, e) => {
                OpcodeKind::WriteBits(Self::update_register_number_in_address(m, a), m[&r], s, e)
            }
            OpcodeKind::WriteOffset(a, v, offset) => OpcodeKind::WriteOffset(
                Self::update_register_number_in_address(m, a),
                Self::update_register_number_in_value(m, v),
                offset,
            ),
        }
    }

    fn update_register_number_in_value(
        m: &HashMap<usize, usize>,
        value: Value<Word>,
    ) -> Value<Word> {
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

    fn remap_blocks(m: &HashMap<usize, usize>, opcode: OpcodeKind<Word>) -> OpcodeKind<Word> {
        match opcode {
            OpcodeKind::Call(a, v) => OpcodeKind::Call(
                Self::remap_blocks_in_address(m, a),
                Self::remap_blocks_in_value(m, v),
            ),
            OpcodeKind::CallLibrary(c, r, i, o) => OpcodeKind::CallLibrary(c, r, i, o),
            OpcodeKind::CheckInputs(r, i, o) => OpcodeKind::CheckInputs(r, i, o),
            OpcodeKind::Load(c) => OpcodeKind::Load(Self::remap_blocks_in_constant(m, c)),
            OpcodeKind::LoadResv => OpcodeKind::LoadResv,
            OpcodeKind::Offset {
                base,
                item_size,
                item_count,
            } => OpcodeKind::Offset {
                base: Self::remap_blocks_in_address(m, base),
                item_size,
                item_count: Self::remap_blocks_in_value(m, item_count),
            },
            OpcodeKind::Read(a) => OpcodeKind::Read(Self::remap_blocks_in_address(m, a)),
            OpcodeKind::ReadBits(a, s, e) => {
                OpcodeKind::ReadBits(Self::remap_blocks_in_address(m, a), s, e)
            }
            OpcodeKind::ReadOffset(a, v) => {
                OpcodeKind::ReadOffset(Self::remap_blocks_in_address(m, a), v)
            }
            OpcodeKind::WriteBits(a, r, s, e) => {
                OpcodeKind::WriteBits(Self::remap_blocks_in_address(m, a), r, s, e)
            }
            OpcodeKind::WriteOffset(a, r, v) => {
                OpcodeKind::WriteOffset(Self::remap_blocks_in_address(m, a), r, v)
            }
        }
    }

    fn remap_blocks_in_value(m: &HashMap<usize, usize>, value: Value<Word>) -> Value<Word> {
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

    fn remap_blocks_in_constant(
        m: &HashMap<usize, usize>,
        value: Constant<Word>,
    ) -> Constant<Word> {
        match value {
            Constant::Address((b, a)) => Constant::Address((m[&b], a)),
            value => value,
        }
    }
}
