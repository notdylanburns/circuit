use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::codegen::register_allocator::{registers, Action, RegisterAllocator};
use crate::codegen::targets::ir::{
    blocks::{Block, BlockAddress},
    get_register_usage, Address, Constant, DataItem, Opcode, OpcodeKind, Sections, Value,
};
use crate::util::{invert_hashmap, run_cmd};

const WORD_SIZE: usize = 8;
const STACK_ALIGNMENT: usize = 2;

registers! {
    Rax(0),
    Rbx(1),
    Rcx(2),
    Rdx(3),
    Rsi(4),
    Rdi(5),
    R8(6),
    R9(7),
    R10(8),
    R11(9),
    R12(10),
    R13(11),
    R14(12),
    R15(13),
}

impl Register {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Rax => "rax",
            Self::Rbx => "rbx",
            Self::Rcx => "rcx",
            Self::Rdx => "rdx",
            Self::Rsi => "rsi",
            Self::Rdi => "rdi",
            Self::R8 => "r8",
            Self::R9 => "r9",
            Self::R10 => "r10",
            Self::R11 => "r11",
            Self::R12 => "r12",
            Self::R13 => "r13",
            Self::R14 => "r14",
            Self::R15 => "r15",
        }
    }
}

impl std::fmt::Display for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.as_str())
    }
}

#[derive(Debug, Default)]
struct Generator {
    labels: HashMap<BlockAddress, Vec<&'static str>>,
    data_section: String,
    rodata_section: String,
    text_section: String,
}

impl Generator {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_block_label(block_id: usize) -> String {
        format!(".LBLK{}", block_id)
    }

    fn get_labels_for_addr(&self, block_address: BlockAddress) -> String {
        let Some(lbls) = self.labels.get(&block_address) else {
            return String::new();
        };

        lbls.iter()
            .map(|lbl| format!("{lbl}:"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn emit_strtab(&mut self, block: &Block<Box<str>>) -> String {
        let label = Self::get_block_label(block.id);
        self.rodata_section.push_str(&format!("{}:\n", label));

        for (addr, item) in block.iter().enumerate() {
            let labels = self.get_labels_for_addr((block.id, addr));
            if !labels.is_empty() {
                self.rodata_section.push_str(&format!("{labels}\n"));
            }
            self.rodata_section
                .push_str(&format!("    .asciz \"{item}\"\n"));
        }

        self.rodata_section.push_str("    .byte 0\n");

        label
    }

    fn emit_rodata(&mut self, block: &Block<u64>) -> String {
        let label = Self::get_block_label(block.id);
        self.rodata_section.push_str(&format!("{}:\n", label));

        let final_index = block.items().len() - 1;

        block
            .iter()
            .enumerate()
            .scan([0u64; 4], |s, (i, &v)| {
                let index = i % 4;
                s[index] = v;
                if index == 3 || i == final_index {
                    Some(Some(*s))
                } else {
                    Some(None)
                }
            })
            .flatten()
            .for_each(|[a, b, c, d]| {
                self.rodata_section.push_str(&format!(
                    "    .quad 0x{a:016x}, 0x{b:016x}, 0x{c:016x}, 0x{d:016x}\n"
                ));
            });

        label
    }

    fn emit_data(&mut self, block: &Block<DataItem<u64>>) -> String {
        let label = Self::get_block_label(block.id);
        self.data_section.push_str(&format!("{}:\n", label));

        for (addr, item) in block.iter().copied().enumerate() {
            let labels = self.get_labels_for_addr((block.id, addr));
            if !labels.is_empty() {
                self.data_section.push_str(&format!("{labels}\n"));
            }

            match item {
                DataItem::Address((blk, addr)) => {
                    let block_label = Self::get_block_label(blk);
                    if addr == 0 {
                        self.data_section
                            .push_str(&format!("    .quad {block_label}\n"));
                    } else {
                        self.data_section
                            .push_str(&format!("    .quad {block_label} + {}\n", addr * WORD_SIZE));
                    }
                }
                DataItem::Word(x) => {
                    self.data_section
                        .push_str(&format!("    .quad 0x{x:016x}\n"));
                }
            }
        }

        label
    }

    fn emit_text(&mut self, block: &Block<Opcode<u64>>) -> String {
        let label = Self::get_block_label(block.id);

        let block_gen = BlockGenerator::new(block, &self.labels);
        let block_opcodes = block_gen.generate();

        self.text_section
            .push_str(&format!("{label}:\n{block_opcodes}\n\n"));

        label
    }

    pub fn generate(mut self, sections: Sections<u64>) -> String {
        self.labels = invert_hashmap(sections.labels.clone());

        for block in sections.data_blocks() {
            self.emit_data(block);
        }

        for block in sections.rodata_blocks() {
            self.emit_rodata(block);
        }

        for block in sections.strtab_blocks() {
            self.emit_strtab(block);
        }

        for block in sections.text_blocks() {
            self.emit_text(block);
        }

        self.to_string()
    }
}

impl std::fmt::Display for Generator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let global_labels = self
            .labels
            .values()
            .flatten()
            .map(|l| format!(".global {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        write!(
            f,
            r#"{}

.section .rodata
{}

.section .data
{}

.section .text
{}"#,
            global_labels, self.rodata_section, self.data_section, self.text_section
        )
    }
}

struct RestoreStack {
    alignment_bytes: usize,
    regs: Vec<Register>,
}

struct BlockGenerator<'b> {
    register_allocator: RegisterAllocator<Register>,
    block: &'b Block<Opcode<u64>>,
    labels: &'b HashMap<BlockAddress, Vec<&'static str>>,
    spilled_word_count: usize,
    opcodes: Vec<String>,
    location: usize,
}

impl<'b> BlockGenerator<'b> {
    fn new(
        block: &'b Block<Opcode<u64>>,
        labels: &'b HashMap<BlockAddress, Vec<&'static str>>,
    ) -> Self {
        let register_usage = get_register_usage(block);
        let register_allocator = RegisterAllocator::new(&register_usage);
        let spilled_word_count = register_allocator.spill_count();

        Self {
            register_allocator,
            block,
            labels,
            spilled_word_count,
            opcodes: vec![],
            location: 0,
        }
    }

    fn get_labels_for_addr(&self, block_address: BlockAddress) -> String {
        let Some(lbls) = self.labels.get(&block_address) else {
            return String::new();
        };

        lbls.iter()
            .map(|lbl| format!("{lbl}:"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn generate(mut self) -> String {
        if self.spilled_word_count > 0 {
            self.emit(format!("sub %rsp, {}", self.spilled_word_count * WORD_SIZE));
        }

        for (location, opcode) in self.block.iter().enumerate() {
            let labels = self.get_labels_for_addr((self.block.id, location));
            if !labels.is_empty() {
                self.emit(format!("{labels}\n"));
            }
            let spills = self.register_allocator.get_spills(location);
            self.location = location;
            self.emit_spills(&spills);
            self.emit_opcode(location, *opcode);
        }

        if self.spilled_word_count > 0 {
            self.emit(format!(
                "add ${}, %rsp",
                self.spilled_word_count * WORD_SIZE
            ));
        }

        self.emit("ret".to_string());

        self.opcodes
            .into_iter()
            .map(|opcode| format!("    {opcode}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn emit(&mut self, opc: String) {
        self.opcodes.push(format!("{opc:<34}# {}", self.location));
    }

    fn emit_spills(&mut self, spills: &[(usize, Register)]) {
        for &(into, reg) in spills {
            if into == 0 {
                self.emit(format!("movq {reg}, %rsp"));
            } else {
                self.emit(format!("movq {reg}, {}(%rsp)", into * WORD_SIZE));
            }
        }
    }

    fn emit_register(&mut self, location: usize, reg: usize) -> String {
        match self
            .register_allocator
            .get_allocation_strategy(location, reg)
        {
            Action::Use(reg) => format!("{reg}"),
            Action::Unspill(mem, reg) => {
                self.emit(format!("movq {}(%rsp), {reg}", mem * WORD_SIZE));
                format!("{reg}")
            }
            Action::Allocate(_) | Action::Spill(..) | Action::None => unreachable!(),
        }
    }

    fn emit_lea<T>(&mut self, dest: T, block_address: BlockAddress) -> String
    where
        T: std::fmt::Display,
    {
        let (blk, addr) = block_address;

        self.emit(format!(
            "lea {}(%rip), {dest}",
            Generator::get_block_label(blk)
        ));
        if addr > 0 {
            self.emit(format!("add ${}, {dest}", addr * WORD_SIZE));
        }
        format!("{dest}")
    }

    fn emit_constant(&mut self, constant: Constant<u64>) -> String {
        match constant {
            Constant::Address((blk, addr)) => {
                format!("{} + {}", Generator::get_block_label(blk), addr * WORD_SIZE)
            }
            Constant::Value(value) => format!("$0x{value:x}"),
        }
    }

    fn emit_value(&mut self, location: usize, value: Value<u64>) -> String {
        match value {
            Value::Immediate(c) => self.emit_constant(c),
            Value::Indirect(Address::Register(reg)) => {
                format!("({})", self.emit_register(location, reg))
            }
            Value::Indirect(Address::Immediate((blk, addr))) => {
                format!(
                    "({} + {})",
                    Generator::get_block_label(blk),
                    addr * WORD_SIZE
                )
            }
            Value::Register(reg) => self.emit_register(location, reg),
        }
    }

    // fn emit_value_no_prefix(&mut self, location: usize, value: Value) -> String {
    //     match value {
    //         Value::Immediate(Constant::Value(value)) => format!("{value}"),
    //         _ => self.emit_value(location, value),
    //     }
    // }

    // fn emit_addr(&mut self, location: usize, addr: Address) -> String {
    //     match addr {
    //         Address::Immediate((blk, 0)) => {
    //             format!("{}(%rip)", Generator::get_block_label(blk))
    //         }
    //         Address::Immediate((blk, addr)) => {
    //             self.emit(format!(
    //                 "lea {}(%rip), {SCRATCH_ADDR}",
    //                 Generator::get_block_label(blk)
    //             ));
    //             format!("{}({SCRATCH_ADDR})", addr * WORD_SIZE)
    //         }
    //         Address::Register(reg) => self.emit_register(location, reg),
    //     }
    // }

    fn push_regs<'a, T>(&mut self, regs: T) -> RestoreStack
    where
        T: IntoIterator<Item = &'a Register>,
    {
        let mut regs: Vec<Register> = regs.into_iter().copied().collect();
        let alignment_bytes = (regs.len() % STACK_ALIGNMENT) * WORD_SIZE;

        if alignment_bytes > 0 {
            self.emit(format!("subq ${alignment_bytes}, %rsp"));
        };

        for reg in regs.iter() {
            self.emit(format!("pushq {reg}"));
        }

        regs.reverse();

        RestoreStack {
            alignment_bytes,
            regs,
        }
    }

    fn restore_stack(&mut self, restorer: RestoreStack) {
        for reg in restorer.regs {
            self.emit(format!("popq {reg}"));
        }

        if restorer.alignment_bytes > 0 {
            self.emit(format!("addq ${}, %rsp", restorer.alignment_bytes));
        }
    }

    fn emit_addr(&mut self, location: usize, addr: Address) -> String {
        match addr {
            Address::Immediate((blk, 0)) => Generator::get_block_label(blk),
            Address::Immediate((blk, addr)) => {
                format!("{} + {addr}", Generator::get_block_label(blk))
            }
            Address::Register(reg) => self.emit_register(location, reg),
        }
    }

    // fn emit_align_stack(&mut self, alignment: usize) {
    //     self.emit(format!("pushq %rsp"));
    //     self.emit(format!("andq $-{}, %rsp", alignment));
    // }

    // fn emit_restore_stack(&mut self) {
    //     self.emit(format!("popq %rsp"));
    // }

    fn emit_opcode(&mut self, location: usize, opcode: Opcode<u64>) {
        let dest = match self
            .register_allocator
            .get_allocation_strategy(location, opcode.destination)
        {
            Action::Allocate(reg) => Some(reg),
            Action::Use(reg) => Some(reg),
            Action::None => None,
            Action::Spill(..) | Action::Unspill(..) => unreachable!(),
        };

        match (dest, opcode.opcode) {
            (None, OpcodeKind::Call(addr, value)) => {
                let regs_in_use = self.register_allocator.get_active_leases(location).to_vec();

                let restorer = self.push_regs(&regs_in_use);

                let value = self.emit_value(location, value);
                // We don't push %rbp inside of functions, so need to align the stack on a multiple
                // of 16 - 8, since we will push %rip when we call
                self.emit(format!("pushq {value}"));
                let addr = self.emit_addr(location, addr);
                self.emit(format!("call {addr}"));
                self.emit(format!("add ${WORD_SIZE}, %rsp"));

                self.restore_stack(restorer);
            }
            (None, OpcodeKind::CallLibrary(c, r, i, o)) => {
                let regs_in_use = self.register_allocator.get_active_leases(location).to_vec();
                // C calling convention Caller Saved Regs + RAX to hold the addr of the function
                // we are calling - we only save ones that are in use
                let caller_saved_regs = regs_in_use.iter().filter(|r| {
                    matches!(
                        *r,
                        Register::Rax
                            | Register::Rdi
                            | Register::Rsi
                            | Register::Rdx
                            | Register::Rcx
                            | Register::R10
                            | Register::R11
                    )
                });

                let restorer = self.push_regs(caller_saved_regs);

                // caller_saved_regs
                //     .clone()
                //     .for_each(|r| self.emit(format!("pushq {r}")));

                let resv = self.emit_register(location, r);
                self.emit(format!("mov {resv}, %rdi"));

                let inputs_addr = self.emit_register(location, i);
                self.emit(format!("mov {inputs_addr}, %rdx"));

                let outputs_addr = self.emit_register(location, o);
                self.emit(format!("mov {outputs_addr}, %rcx"));

                self.emit("movq $RUNTIME_STATE, %rsi".to_string());

                self.emit(format!(
                    "movq _func_table+{}(%rip), %rax",
                    ((2 * c) + 1) * WORD_SIZE
                ));

                // self.emit_align_stack(16);

                self.emit("call *%rax".to_string());

                self.restore_stack(restorer);

                // self.emit_restore_stack();

                // caller_saved_regs
                //     .rev()
                //     .for_each(|r| self.emit(format!("popq {r}")));
            }
            (None, OpcodeKind::CheckInputs(r, i, o)) => (),
            (Some(dest), OpcodeKind::Load(Constant::Address(a))) => {
                self.emit_lea(dest, a);
            }
            (Some(dest), OpcodeKind::Load(Constant::Value(value))) => {
                self.emit(format!("movq ${value}, {dest}"));
            }
            (Some(dest), OpcodeKind::LoadResv) => {
                self.emit(format!("mov {}(%rsp), {dest}", WORD_SIZE))
            }
            (
                Some(dest),
                OpcodeKind::Offset {
                    base,
                    item_size,
                    item_count,
                },
            ) => {
                let item_size = item_size * WORD_SIZE;

                match (base, item_count) {
                    (Address::Register(r), Value::Register(i)) => {
                        let base = self.emit_register(location, r);
                        let item_count = self.emit_register(location, i);
                        self.emit(format!("lea ({base}, {item_count}, {item_size}), {dest}"));
                    }
                    (Address::Register(r), Value::Immediate(Constant::Value(i))) => {
                        let base = self.emit_register(location, r);
                        self.emit(format!("lea {}({base}), {dest}", item_size as u64 * i))
                    }
                    (Address::Immediate((blk, addr)), Value::Register(r)) => {
                        let item_count = self.emit_register(location, r);
                        self.emit(format!("movq ${item_size}, {dest}"));
                        self.emit(format!("imulq {item_count}, {dest}"));
                        self.emit(format!(
                            "lea {}+{addr}({dest}), {dest}",
                            Generator::get_block_label(blk)
                        ));
                    }
                    _ => todo!("{base} {item_count}"),
                }
            }
            (Some(dest), OpcodeKind::Read(addr)) => {
                let addr = self.emit_addr(location, addr);
                self.emit(format!("movq ({addr}), {dest}"));
            }
            (Some(dest), OpcodeKind::ReadBits(addr, start, end)) => {
                let addr = self.emit_addr(location, addr);
                self.emit(format!("movq ({addr}), {dest}"));
                let bits = end - start;
                let mask = if bits * 2 == WORD_SIZE * 8 {
                    usize::MAX
                } else {
                    (2usize.pow(2 * bits as u32) - 1) << (start * 2)
                };
                self.emit(format!("and $0x{mask:x}, {dest}"));

                if start != 0 {
                    self.emit(format!("shr ${}, {dest}", start * 2));
                }
            }
            (Some(dest), OpcodeKind::ReadOffset(addr, offset)) => {
                let addr = self.emit_addr(location, addr);
                self.emit(format!("movq {}({addr}), {dest}", offset * WORD_SIZE));
            }
            (None, OpcodeKind::WriteBits(addr, reg, start, _)) => {
                let addr = self.emit_addr(location, addr);
                let reg = self.emit_register(location, reg);

                if start != 0 {
                    self.emit(format!("shl ${}, {reg}", start * 2));
                }

                self.emit(format!("orq {reg}, ({addr})"));
            }
            (None, OpcodeKind::WriteOffset(addr, value, offset)) => {
                let addr = self.emit_addr(location, addr);
                let reg = self.emit_value(location, value);

                if offset > 0 {
                    self.emit(format!("mov {reg}, {offset}({addr})"));
                } else {
                    self.emit(format!("mov {reg}, ({addr})"));
                }
            }
            (
                None,
                OpcodeKind::Load(..)
                | OpcodeKind::LoadResv
                | OpcodeKind::Offset { .. }
                | OpcodeKind::Read(..)
                | OpcodeKind::ReadBits(..)
                | OpcodeKind::ReadOffset(..),
            ) => (), // unused result, drop opcode
            (dest, opc) => unreachable!("{dest:?} = {opc}"),
        }
    }
}

fn assemble(source: &Path, output: &Path, debug: bool) {
    let mut cmd = &mut std::process::Command::new("as");
    cmd = if debug { cmd.arg("-g") } else { cmd };

    run_cmd(cmd.arg("-o").arg(output).arg(source));
}

fn link(sources: &[&Path], output: &Path) {
    run_cmd(
        std::process::Command::new("cc")
            .arg("-no-pie")
            .arg("-fsanitize=address")
            .arg("-ldl")
            .arg("-g")
            .arg("-o")
            .arg(output)
            .args(sources),
    );
}

pub fn generate(blocks: Sections<u64>, output_file_name: &str) {
    let generator = Generator::new();
    let asm = generator.generate(blocks);

    let tmp_dir = std::env::temp_dir();
    let asm_path = tmp_dir.join(format!("{output_file_name}.s"));
    let obj_path = tmp_dir.join(format!("{output_file_name}.o"));
    let rt_path = PathBuf::from("/usr/local/lib/cktrt.o");

    match std::fs::write(&asm_path, asm) {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "[FATAL] failed to write assembly to {}: {e}",
                asm_path.display()
            );
            std::process::exit(1);
        }
    }

    assemble(&asm_path, &obj_path, true);
    link(&[&obj_path, &rt_path], &PathBuf::from(output_file_name));
}
