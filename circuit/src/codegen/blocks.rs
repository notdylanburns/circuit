use std::hash::Hash;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BlockType {
    Code,
    Data,
    Resv,
}

impl std::fmt::Display for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "CODE"),
            Self::Data => write!(f, "DATA"),
            Self::Resv => write!(f, "RESV"),
        }
    }
}

pub type BlockAddress = (usize, usize);

#[derive(Debug, PartialEq, Eq)]
pub struct Block<T> {
    pub id: usize,
    block_type: BlockType,
    items: Vec<T>,
}

impl<T> Block<T> {
    fn new(id: usize, block_type: BlockType) -> Self {
        Self {
            id,
            block_type,
            items: vec![],
        }
    }

    pub fn replace_items(&self, items: Vec<T>) -> Self {
        Self {
            id: self.id,
            block_type: self.block_type,
            items,
        }
    }

    pub fn base_addr(&self) -> BlockAddress {
        (self.id, 0)
    }

    pub fn offset(&self) -> usize {
        self.items.len()
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
        self.items.extend(items.into_iter());
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

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> {
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
        (&self.block_type, &self.items).hash(state)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Block {}: {}", self.id, self.block_type)?;
        let index_size = f64::log10(self.items.len() as f64) as usize;
        for (index, item) in self.items.iter().enumerate() {
            writeln!(f, "{index:<index_size$}: {item}")?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Blocks<C, D, R = D> {
    code_blocks: Vec<Block<C>>,
    data_blocks: Vec<Block<D>>,
    resv_blocks: Vec<Block<R>>,
    block_types: Vec<(BlockType, usize)>,
    entry: Option<BlockAddress>,
    main_resv: Option<BlockAddress>,
}

impl<C, D, R> Default for Blocks<C, D, R> {
    fn default() -> Self {
        Self {
            code_blocks: vec![],
            data_blocks: vec![],
            resv_blocks: vec![],
            block_types: vec![],
            entry: None,
            main_resv: None,
        }
    }
}

impl<C, D, R> Blocks<C, D, R> {
    fn next_id(&mut self, block_type: BlockType) -> usize {
        let next_id = self.block_types.len();
        let blk_type_id = match block_type {
            BlockType::Code => self.code_blocks.len(),
            BlockType::Data => self.data_blocks.len(),
            BlockType::Resv => self.resv_blocks.len(),
        };

        self.block_types.push((block_type, blk_type_id));

        next_id
    }

    pub fn len(&self) -> usize {
        self.block_types.len()
    }

    pub fn new_code(&mut self) -> &mut Block<C> {
        let next_block_id = self.next_id(BlockType::Code);
        self.code_blocks
            .push(Block::new(next_block_id, BlockType::Code));

        self.code_blocks.last_mut().unwrap()
    }

    pub fn push_code(&mut self, items: Vec<C>) -> usize {
        let id = self.next_id(BlockType::Code);
        self.code_blocks.push(Block {
            id,
            block_type: BlockType::Code,
            items,
        });

        id
    }

    pub fn new_data(&mut self) -> &mut Block<D> {
        let next_block_id = self.next_id(BlockType::Data);
        self.data_blocks
            .push(Block::new(next_block_id, BlockType::Data));

        self.data_blocks.last_mut().unwrap()
    }

    pub fn push_data(&mut self, items: Vec<D>) -> usize {
        let id = self.next_id(BlockType::Data);
        self.data_blocks.push(Block {
            id,
            block_type: BlockType::Data,
            items,
        });

        id
    }

    pub fn new_resv(&mut self) -> &mut Block<R> {
        let next_block_id = self.next_id(BlockType::Resv);
        self.resv_blocks
            .push(Block::new(next_block_id, BlockType::Resv));

        self.resv_blocks.last_mut().unwrap()
    }

    pub fn push_resv(&mut self, items: Vec<R>) -> usize {
        let id = self.next_id(BlockType::Resv);
        self.resv_blocks.push(Block {
            id,
            block_type: BlockType::Resv,
            items,
        });

        id
    }

    pub fn get_block_type(&self, id: usize) -> (BlockType, usize) {
        self.block_types[id]
    }

    pub fn get_data_block(&self, id: usize) -> &Block<D> {
        match self.get_block_type(id) {
            (BlockType::Data, data_id) => &self.data_blocks[data_id],
            (BlockType::Code, _) => unreachable!("expected data block, found code block"),
            (BlockType::Resv, _) => unreachable!("expected data block, found reserved block"),
        }
    }

    pub fn code_blocks<'a>(&'a self) -> impl Iterator<Item = &'a Block<C>> {
        self.code_blocks.iter()
    }

    pub fn code_blocks_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut Block<C>> {
        self.code_blocks.iter_mut()
    }

    pub fn data_blocks<'a>(&'a self) -> impl Iterator<Item = &'a Block<D>> {
        self.data_blocks.iter()
    }

    pub fn resv_blocks<'a>(&'a self) -> impl Iterator<Item = &'a Block<R>> {
        self.resv_blocks.iter()
    }

    pub fn resv_blocks_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut Block<R>> {
        self.resv_blocks.iter_mut()
    }

    pub fn set_entry(&mut self, addr: BlockAddress) {
        self.entry.replace(addr);
    }

    pub fn get_entry(&self) -> BlockAddress {
        self.entry.unwrap()
    }

    pub fn set_main_resv(&mut self, addr: BlockAddress) {
        self.main_resv.replace(addr);
    }

    pub fn get_main_resv(&self) -> BlockAddress {
        self.main_resv.unwrap()
    }
}

impl<C, D, R> std::fmt::Display for Blocks<C, D, R>
where
    C: std::fmt::Display,
    D: std::fmt::Display,
    R: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Entry Point: Block {}", self.get_entry().0)?;
        for (bt, id) in self.block_types.iter().copied() {
            match bt {
                BlockType::Code => writeln!(f, "{}", self.code_blocks[id])?,
                BlockType::Data => writeln!(f, "{}", self.data_blocks[id])?,
                BlockType::Resv => writeln!(f, "{}", self.resv_blocks[id])?,
            }
        }

        Ok(())
    }
}
