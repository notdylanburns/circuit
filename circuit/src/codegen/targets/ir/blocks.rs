use crate::codegen::targets::ir::{DataItem, WordTrait};

use super::Opcode;
use std::{collections::HashMap, hash::Hash};

pub type BlockAddress = (usize, usize);

#[derive(Debug, PartialEq, Eq)]
pub struct Block<T> {
    pub id: usize,
    section: Section,
    items: Vec<T>,
}

impl<T> Block<T> {
    fn new(id: usize, section: Section) -> Self {
        Self {
            id,
            section,
            items: vec![],
        }
    }

    pub fn replace_items(&self, items: Vec<T>) -> Self {
        Self {
            id: self.id,
            section: self.section,
            items,
        }
    }

    pub fn base_addr(&self) -> BlockAddress {
        (self.id, 0)
    }

    pub fn offset(&self) -> usize {
        self.items.len()
    }

    pub fn addr(&self) -> BlockAddress {
        (self.id, self.items.len())
    }

    pub fn push(&mut self, item: T) -> BlockAddress {
        let address = self.items.len();
        self.items.push(item);
        (self.id, address)
    }

    pub fn extend(&mut self, items: &[T]) -> BlockAddress
    where
        T: Copy,
    {
        let address = self.items.len();
        self.items.extend(items);
        (self.id, address)
    }

    pub fn reserve(&mut self, length: usize) -> BlockAddress
    where
        T: Default + Copy,
    {
        let address = self.items.len();
        self.items.append(&mut vec![T::default(); length]);
        (self.id, address)
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn map_items<F>(&mut self, f: F)
    where
        F: Fn(&T) -> T,
    {
        self.items = self.items.iter().map(f).collect()
    }
}

impl<T: Hash> Hash for Block<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (&self.section, &self.items).hash(state)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Block {}: {}", self.id, self.section)?;
        let index_size = f64::log10(self.items.len() as f64) as usize;
        for (index, item) in self.items.iter().enumerate() {
            writeln!(f, "{index:<index_size$}: {item}")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Text,
    Rodata,
    Data,
    Strtab,
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data => write!(f, "data"),
            Self::Rodata => write!(f, "rodata"),
            Self::Strtab => write!(f, "strtab"),
            Self::Text => write!(f, "text"),
        }
    }
}

#[derive(Debug)]
pub struct Sections<Word: WordTrait> {
    pub labels: HashMap<&'static str, BlockAddress>,
    text: Vec<Block<Opcode<Word>>>,
    rodata: Vec<Block<Word>>,
    data: Vec<Block<DataItem<Word>>>,
    strtab: Vec<Block<Box<str>>>,
    block_types: Vec<(Section, usize)>,
}

impl<Word: WordTrait> Default for Sections<Word> {
    fn default() -> Self {
        Self {
            labels: HashMap::new(),
            text: vec![],
            rodata: vec![],
            data: vec![],
            strtab: vec![],
            block_types: vec![],
        }
    }
}

impl<Word: WordTrait> Sections<Word> {
    fn next_id(&mut self, section: Section) -> usize {
        let next_id = self.len();
        let section_id = match section {
            Section::Text => self.text.len(),
            Section::Data => self.data.len(),
            Section::Rodata => self.rodata.len(),
            Section::Strtab => self.strtab.len(),
        };

        self.block_types.push((section, section_id));
        assert_eq!(next_id, self.block_types.len() - 1);

        next_id
    }

    pub fn len(&self) -> usize {
        self.block_types.len()
    }

    pub fn get_block_section(&self, id: usize) -> (Section, usize) {
        self.block_types[id]
    }

    pub fn label(&mut self, name: &'static str, address: BlockAddress) {
        if let Some(old_address) = self.labels.insert(name, address) {
            unreachable!(
                "warning: label `{name}` already exists at b{}:{}; overwriting with b{}:{}",
                old_address.0, old_address.1, address.0, address.1
            );
        };
    }

    pub fn get_label(&self, name: &str) -> Option<BlockAddress> {
        self.labels.get(name).copied()
    }
}

macro_rules! section_impl {
    (
        $section:ident,
        $item:ty,
        $enum:ident,
        $new_fn:ident,
        $push_fn:ident,
        $get_fn:ident,
        $get_mut_fn:ident,
        $iter_fn:ident,
        $iter_mut_fn:ident,
    ) => {
        impl<Word: WordTrait> Sections<Word> {
            pub fn $new_fn(&mut self) -> &mut Block<$item> {
                let next_block_id = self.next_id(Section::$enum);
                self.$section
                    .push(Block::new(next_block_id, Section::$enum));

                self.$section.last_mut().unwrap()
            }

            pub fn $push_fn(&mut self, items: Vec<$item>) -> usize {
                let id = self.next_id(Section::$enum);
                self.$section.push(Block {
                    id,
                    section: Section::$enum,
                    items,
                });

                id
            }

            pub fn $get_fn(&self, id: usize) -> &Block<$item> {
                match self.block_types[id] {
                    (Section::$enum, section_id) => &self.$section[section_id],
                    _ => unreachable!(
                        "expected {} block, found different type",
                        stringify!($section)
                    ),
                }
            }

            pub fn $get_mut_fn(&mut self, id: usize) -> &mut Block<$item> {
                match self.block_types[id] {
                    (Section::$enum, section_id) => &mut self.$section[section_id],
                    _ => unreachable!(
                        "expected {} block, found different type",
                        stringify!($section)
                    ),
                }
            }

            pub fn $iter_fn(&self) -> impl Iterator<Item = &'_ Block<$item>> {
                self.$section.iter()
            }

            pub fn $iter_mut_fn(&mut self) -> impl Iterator<Item = &'_ mut Block<$item>> {
                self.$section.iter_mut()
            }
        }
    };
}

section_impl!(
    text,
    Opcode<Word>,
    Text,
    new_text,
    push_text,
    get_text,
    get_text_mut,
    text_blocks,
    text_blocks_mut,
);

section_impl!(
    data,
    DataItem<Word>,
    Data,
    new_data,
    push_data,
    get_data,
    get_data_mut,
    data_blocks,
    data_blocks_mut,
);

section_impl!(
    rodata,
    Word,
    Rodata,
    new_rodata,
    push_rodata,
    get_rodata,
    get_rodata_mut,
    rodata_blocks,
    rodata_blocks_mut,
);

section_impl!(
    strtab,
    Box<str>,
    Strtab,
    new_strtab,
    push_strtab,
    get_strtab,
    get_strtab_mut,
    strtab_blocks,
    strtab_blocks_mut,
);

impl<Word: WordTrait> std::fmt::Display for Sections<Word>
where
    Word: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "section .rodata")?;
        for b in self.rodata_blocks() {
            writeln!(f, "{}", b)?;
        }

        writeln!(f, "section .rodata.strtab")?;
        for b in self.strtab_blocks() {
            writeln!(f, "{}", b)?;
        }

        writeln!(f, "section .data")?;
        for b in self.data_blocks() {
            writeln!(f, "{}", b)?;
        }

        writeln!(f, "section .text")?;
        for b in self.text_blocks() {
            writeln!(f, "{}", b)?;
        }

        Ok(())
    }
}
