use std::collections::HashMap;
use std::hash::Hash;

mod chain_map;
pub use chain_map::ChainMap;

mod interner;
pub use interner::Interner;

mod linked;

use crate::loader::ModuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibrarySymbolType {
    Circ,
    Const,
    Enum,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pos {
    None,
    Builtin,
    Module(ModuleId),
    Library(ModuleId, LibrarySymbolType, usize),
    Pos {
        module_id: ModuleId,
        line: usize,
        col: usize,
        start: usize,
        span: usize,
    },
}

impl Default for Pos {
    fn default() -> Self {
        Self::None
    }
}

impl Pos {
    pub fn new(module_id: ModuleId, line: usize, col: usize, start: usize, span: usize) -> Self {
        Self::Pos {
            module_id,
            line,
            col,
            start,
            span,
        }
    }
}

pub trait Position {
    fn pos(&self) -> Pos;
}

impl Pos {
    pub fn module_id(&self) -> Option<ModuleId> {
        match self {
            Self::Builtin | Self::None => None,
            Self::Module(module_id) | Self::Library(module_id, ..) => Some(*module_id),
            Self::Pos { module_id, .. } => Some(*module_id),
        }
    }

    pub fn loc(&self) -> Option<(usize, usize)> {
        if let Self::Pos { line, col, .. } = self {
            Some((*line, *col))
        } else {
            None
        }
    }

    pub fn line(&self) -> Option<usize> {
        if let Self::Pos { line, .. } = self {
            Some(*line)
        } else {
            None
        }
    }

    pub fn col(&self) -> Option<usize> {
        if let Self::Pos { col, .. } = self {
            Some(*col)
        } else {
            None
        }
    }

    pub fn start(&self) -> Option<usize> {
        if let Self::Pos { start, .. } = self {
            Some(*start)
        } else {
            None
        }
    }

    pub fn span(&self) -> Option<usize> {
        if let Self::Pos { span, .. } = self {
            Some(*span)
        } else {
            None
        }
    }

    pub fn end(&self) -> Option<usize> {
        if let Self::Pos { start, span, .. } = self {
            Some(*start + *span)
        } else {
            None
        }
    }

    pub fn expand(self, other: Pos) -> Self {
        let Self::Pos {
            module_id,
            line,
            col,
            start,
            ..
        } = self
        else {
            unreachable!("expand called on invalid pos: {self:#?}")
        };

        let end = other
            .end()
            .unwrap_or_else(|| unreachable!("expand called with invalid pos: {other:#?}"));

        Pos::new(module_id, line, col, start, end - start)
    }
}

impl Position for Pos {
    fn pos(&self) -> Pos {
        *self
    }
}

#[derive(Debug)]
pub struct OrderedMap<K: Eq + Hash, V> {
    map: HashMap<K, usize>,
    items: Vec<V>,
}

impl<K: Hash + Eq, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            items: Vec::new(),
        }
    }
}

#[allow(dead_code)]
impl<K: Eq + Hash, V> OrderedMap<K, V> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            items: Vec::new(),
        }
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.contains_key(key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.get(key).map(|index| &self.items[*index])
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.get_mut(key).map(|index| &mut self.items[*index])
    }

    pub fn get_index<Q>(&self, key: &Q) -> Option<usize>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.get(key).map(|index| *index)
    }

    pub fn get_entry<Q>(&self, key: &Q) -> Option<(usize, &V)>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.get(key).map(|index| (*index, &self.items[*index]))
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<usize, V> {
        match self.map.get(&key) {
            Some(index) => Err(std::mem::replace(&mut self.items[*index], value)),
            None => {
                let index = self.items.len();
                self.items.push(value);
                self.map.insert(key, index);
                Ok(index)
            }
        }
    }

    pub fn insert_new(&mut self, key: K, value: V) -> bool {
        if let Some(_) = self.get(&key) {
            false
        } else {
            match self.insert(key, value) {
                Ok(_) => true,
                Err(_) => unreachable!(),
            }
        }
    }

    pub fn get_by_index(&self, index: usize) -> Option<&V> {
        self.items.get(index)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.items.iter()
    }

    pub fn value_indices(&self) -> impl Iterator<Item = (usize, &V)> {
        self.items.iter().enumerate()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.items.iter_mut()
    }

    pub fn into_values(self) -> Vec<V> {
        self.items
    }
}

macro_rules! extract {
    ($expr:expr, $pattern:path { $field:ident } $(, $message:literal)?) => {
        match ($expr) {
            $pattern { $field, .. } => $field,
            _ => unreachable!($($message)?),
        }
    };
    ($expr:expr, $pattern:path { $($field:ident),+ } $(, $message:literal)?) => {
        match ($expr) {
            $pattern { $($field,)+ .. } => ($($field),+),
            _ => unreachable!($($message)?),
        }
    };
    ($expr:expr, $pattern:path [ $field:ident ] $(, $message:literal)?) => {
        match ($expr) {
            $pattern ( $field, .. ) => $field,
            _ => unreachable!($($message)?),
        }
    };
    ($expr:expr, $pattern:path [ $($field:ident),+ ] $(, $message:literal)?) => {
        match ($expr) {
            $pattern ( $($field,)+ .. ) => ($($field),+),
            _ => unreachable!($($message)?),
        }
    };
}

macro_rules! difference {
    ($lhs:expr, $rhs:expr) => {{
        let lhs = ($lhs);
        let rhs = ($rhs);

        if lhs > rhs {
            lhs - rhs
        } else {
            rhs - lhs
        }
    }};
}

pub fn div_up(a: usize, b: usize) -> usize {
    (0..a).step_by(b).size_hint().0
}

pub fn invert_hashmap<K, V: Eq + Hash>(hm: HashMap<K, V>) -> HashMap<V, Vec<K>> {
    let mut new = HashMap::<V, Vec<K>>::new();

    for (k, v) in hm {
        new.entry(v).or_default().push(k);
    }

    new
}

pub fn transpose<T, U>(m: &[U]) -> Vec<Vec<T>>
where
    U: AsRef<[T]>,
    T: Clone,
{
    let cols = m.len();
    let rows = m.iter().map(|x| x.as_ref().len()).max().unwrap_or(0);

    let mut row_vec = vec![Vec::with_capacity(cols); rows];

    for row in m.iter() {
        for (j, item) in row.as_ref().iter().enumerate() {
            row_vec[j].push(item.clone());
        }
    }

    row_vec
}

fn print_cmd(cmd: &std::process::Command) {
    let cmd = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    println!("[INFO] {cmd}")
}

pub fn run_cmd(cmd: &mut std::process::Command) {
    print_cmd(cmd);
    let prog = cmd.get_program().to_str().unwrap().to_string();

    let status = cmd
        .spawn()
        .unwrap_or_else(|_| {
            eprintln!("[FATAL] failed to spawn '{prog}' process");
            std::process::exit(1);
        })
        .wait()
        .unwrap_or_else(|_| {
            eprintln!("[FATAL] failed to wait for '{prog}' process");
            std::process::exit(1);
        });

    if !status.success() {
        match status.code() {
            Some(code) => eprintln!("[FATAL] '{prog}' failed with exit code: {code}"),
            None => eprintln!("[FATAL] '{prog}' terminated by signal"),
        }
        std::process::exit(1);
    };
}

pub(super) use {difference, extract};

#[cfg(test)]
mod test {
    #[test]
    fn test_extract() {
        enum TestEnum {
            TupleOne(usize),
            TupleMany(usize, usize),
            StructOne { x: usize },
            StructMany { x: usize, y: usize },
        }

        let tuple_one = TestEnum::TupleOne(1);
        let tuple_many = TestEnum::TupleMany(1, 2);
        let struct_one = TestEnum::StructOne { x: 1 };
        let struct_many = TestEnum::StructMany { x: 1, y: 2 };

        assert_eq!(extract!(struct_one, TestEnum::StructOne { x }), 1);
        assert_eq!(extract!(struct_many, TestEnum::StructMany { y }), 2);
        assert_eq!(extract!(struct_many, TestEnum::StructMany { x, y }), (1, 2));
        assert_eq!(extract!(struct_many, TestEnum::StructMany { y, x }), (2, 1));
        assert_eq!(extract!(tuple_one, TestEnum::TupleOne[a]), 1);
        assert_eq!(extract!(tuple_many, TestEnum::TupleMany[a, b]), (1, 2));
    }

    #[test]
    fn test_chainmap() {
        use super::ChainMap;
        use std::collections::HashMap;

        let mut chain_map: ChainMap<&'static str, u8> = ChainMap::new();

        chain_map.push_chain(HashMap::from([("a", 1), ("b", 2)]));

        assert_eq!(chain_map.get(&"a"), Some(&1));
        assert_eq!(chain_map.get(&"b"), Some(&2));

        chain_map.push_chain(HashMap::from([("a", 2), ("c", 5)]));

        assert_eq!(chain_map.get(&"a"), Some(&2));
        assert_eq!(chain_map.get(&"b"), Some(&2));
        assert_eq!(chain_map.get(&"c"), Some(&5));

        chain_map.pop_chain();

        assert_eq!(chain_map.get(&"a"), Some(&1));
        assert_eq!(chain_map.get(&"b"), Some(&2));

        chain_map.pop_chain();

        assert_eq!(chain_map.get(&"a"), None);
        assert_eq!(chain_map.get(&"b"), None);
    }
}
