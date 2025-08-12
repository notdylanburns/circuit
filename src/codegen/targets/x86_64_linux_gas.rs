use std::path::{Path, PathBuf};

use crate::codegen::blocks::{Block, BlockAddress};
use crate::codegen::register_allocator::{registers, Action, RegisterAllocator, Registers};
use crate::codegen::targets::ir::{
    get_register_usage, Address, Constant, IrBlocks, Opcode, OpcodeKind, ResvItem, Value,
};

const WORD_SIZE: usize = 8;

registers! {
    RAX(0),
    RBX(1),
    RCX(2),
    RDX(3),
    RSI(4),
    RDI(5),
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
            Self::RAX => "rax",
            Self::RBX => "rbx",
            Self::RCX => "rcx",
            Self::RDX => "rdx",
            Self::RSI => "rsi",
            Self::RDI => "rdi",
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
    data_section: String,
    code_section: String,
    resv_section: String,
}

impl Generator {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_block_label(block_id: usize) -> String {
        format!(".LBLK{}", block_id)
    }

    fn emit_data(&mut self, block: &Block<usize>) -> String {
        let label = Self::get_block_label(block.id);
        self.data_section.push_str(&format!("{}:\n", label));

        let final_index = block.items().len() - 1;

        block
            .iter()
            .enumerate()
            .scan([0usize; 4], |s, (i, &v)| {
                let index = i % 4;
                s[index] = v;
                if index == 3 || i == final_index {
                    Some(Some(s.clone()))
                } else {
                    Some(None)
                }
            })
            .filter_map(|x| x)
            .for_each(|[a, b, c, d]| {
                self.data_section.push_str(&format!(
                    "    .quad 0x{a:016x}, 0x{b:016x}, 0x{c:016x}, 0x{d:016x}\n"
                ));
            });

        label
    }

    fn emit_resv(&mut self, block: &Block<ResvItem>, is_main: bool) -> String {
        if is_main {
            self.resv_section.push_str("_MAIN_RESV: ");
        }

        let label = Self::get_block_label(block.id);
        self.resv_section.push_str(&format!("{}:\n", label));

        for &item in block.iter() {
            match item {
                ResvItem::BlockAddress((blk, addr)) => {
                    let block_label = Self::get_block_label(blk);
                    if addr == 0 {
                        self.resv_section
                            .push_str(&format!("    .quad {block_label}\n"));
                    } else {
                        self.resv_section
                            .push_str(&format!("    .quad {block_label} + {}\n", addr * WORD_SIZE));
                    }
                }
                ResvItem::Word(x) => {
                    self.resv_section
                        .push_str(&format!("    .quad 0x{x:016x}\n"));
                }
            }
        }

        label
    }

    fn emit_code(&mut self, block: &Block<Opcode>, is_main: bool) -> String {
        if is_main {
            self.code_section.push_str("_MAIN_CODE: ");
        }

        let label = Self::get_block_label(block.id);

        let block_gen = BlockGenerator::new(block);
        let block_opcodes = block_gen.generate();

        self.code_section
            .push_str(&format!("{label}:\n{block_opcodes}\n\n"));

        label
    }

    pub fn generate(mut self, blocks: IrBlocks) -> String {
        for block in blocks.data_blocks() {
            self.emit_data(block);
        }

        for block in blocks.resv_blocks() {
            self.emit_resv(block, blocks.get_main_resv().0 == block.id);
        }

        for block in blocks.code_blocks() {
            self.emit_code(block, blocks.get_entry().0 == block.id);
        }

        self.to_string()
    }
}

impl std::fmt::Display for Generator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#".section .rodata
{}

.section .data
.global _MAIN_RESV
{}

.section .text
.global _MAIN_CODE
{}"#,
            self.data_section, self.resv_section, self.code_section
        )
    }
}

struct BlockGenerator<'b> {
    register_allocator: RegisterAllocator<Register>,
    block: &'b Block<Opcode>,
    spilled_word_count: usize,
    opcodes: Vec<String>,
    location: usize,
}

impl<'b> BlockGenerator<'b> {
    fn new(block: &'b Block<Opcode>) -> Self {
        let register_usage = get_register_usage(&block);
        let register_allocator = RegisterAllocator::new(&register_usage);
        let spilled_word_count = register_allocator.spill_count();

        Self {
            register_allocator,
            block,
            spilled_word_count,
            opcodes: vec![],
            location: 0,
        }
    }

    fn generate(mut self) -> String {
        if self.spilled_word_count > 0 {
            self.emit(format!("sub %rsp, {}", self.spilled_word_count * WORD_SIZE));
        }

        for (location, opcode) in self.block.iter().enumerate() {
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
        self.opcodes.push(format!("{opc:<30}# {}", self.location));
    }

    fn emit_preamble(&mut self) {
        // self.emit(format!("movq {WORD_SIZE}(%rsp), {SELF}"));
        if self.spilled_word_count > 0 {
            self.emit(format!("sub %rsp, {}", self.spilled_word_count * WORD_SIZE));
        }
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

    fn emit_constant(&mut self, constant: Constant) -> String {
        match constant {
            Constant::Address((blk, addr)) => {
                format!("{} + {}", Generator::get_block_label(blk), addr * WORD_SIZE)
            }
            Constant::Value(value) => format!("$0x{value:x}"),
        }
    }

    fn emit_value(&mut self, location: usize, value: Value) -> String {
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

    fn emit_addr(&mut self, location: usize, addr: Address) -> String {
        match addr {
            Address::Immediate((blk, 0)) => Generator::get_block_label(blk),
            Address::Immediate((blk, addr)) => {
                format!("{} + {addr}", Generator::get_block_label(blk))
            }
            Address::Register(reg) => self.emit_register(location, reg),
        }
    }

    fn emit_opcode(&mut self, location: usize, opcode: Opcode) {
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

                regs_in_use
                    .iter()
                    .for_each(|r| self.emit(format!("pushq {r}")));

                let value = self.emit_value(location, value);
                self.emit(format!("push {value}"));
                let addr = self.emit_addr(location, addr);
                self.emit(format!("call {addr}"));
                self.emit(format!("add ${WORD_SIZE}, %rsp"));

                regs_in_use
                    .into_iter()
                    .rev()
                    .for_each(|r| self.emit(format!("popq {r}")));
            }
            (None, OpcodeKind::CheckInputs(r, i, o)) => (),
            (Some(dest), OpcodeKind::Load(Constant::Address(a))) => {
                self.emit_lea(dest, a);
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
                        self.emit(format!("lea {}({base}), {dest}", item_size * i))
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
                    (2usize.pow(2 * bits as u32) - 1) << start * 2
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
                    self.emit(format!("shl ${start}, {reg}"));
                }

                self.emit(format!("orq {reg}, ({addr})"));
            }
            (None, OpcodeKind::WriteOffset(addr, value, offset)) => {
                let addr = self.emit_addr(location, addr);
                let reg = self.emit_value(location, value);

                if offset > 0 {
                    self.emit(format!("orq {reg}, {offset}({addr})"));
                } else {
                    self.emit(format!("orq {reg}, ({addr})"));
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

    let status = cmd
        .arg("-o")
        .arg(&output)
        .arg(&source)
        .spawn()
        .expect("failed to spawn assembler")
        .wait()
        .expect("failed to assemble");

    if !status.success() {
        eprintln!("'as' failed with status: {:?}", status);
        std::process::exit(1);
    };
}

fn link(sources: &[&Path], output: &Path) {
    let status = std::process::Command::new("ld")
        .arg("-o")
        .arg(output)
        .args(sources)
        .spawn()
        .expect("failed to spawn linker")
        .wait()
        .expect("failed to link");

    if !status.success() {
        eprintln!("'ld' failed with status: {:?}", status);
        std::process::exit(1);
    };
}

pub fn generate(blocks: IrBlocks, output_file_name: &str) {
    let generator = Generator::new();
    let asm = generator.generate(blocks);

    let tmp_dir = std::env::temp_dir();
    let asm_path = tmp_dir.join(format!("{output_file_name}.s"));
    let obj_path = tmp_dir.join(format!("{output_file_name}.o"));
    let rt_path = std::env::current_dir()
        .expect("failed to get cwd")
        .join("src/codegen/targets/x86_64_linux_gas/runtime.s");
    let rt_obj_path = tmp_dir.join("x86_64-linux-gas-runtime.o");

    std::fs::write(&asm_path, &asm).expect("failed to write assembly code");

    assemble(&asm_path, &obj_path, true);
    assemble(&rt_path, &rt_obj_path, true);

    link(&[&obj_path, &rt_obj_path], &PathBuf::from(output_file_name));
}
