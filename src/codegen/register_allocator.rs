use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};

pub trait Registers: Hash + Eq + PartialEq + Clone + Copy + Sized + Debug {
    fn count() -> usize;
    fn all() -> Vec<Self>;
    fn order(&self) -> usize;
}

#[derive(Debug, Clone, Copy)]
pub enum Action<T: Registers> {
    None,
    Spill(usize, T),
    Unspill(usize, T),
    Allocate(T),
    Use(T),
}

struct AllocatorState<T: Registers> {
    spills_in_use: HashMap<usize, usize>,
    spills_unused: HashSet<usize>,
    registers_in_use: HashSet<T>,
    registers_unused: HashSet<T>,
}

impl<T: Registers> AllocatorState<T> {
    fn new() -> Self {
        Self {
            spills_in_use: HashMap::new(),
            spills_unused: HashSet::new(),
            registers_in_use: HashSet::new(),
            registers_unused: T::all().into_iter().collect(),
        }
    }

    fn get_unused_register(&mut self) -> Option<T> {
        let &reg = self
            .registers_unused
            .iter()
            .min_by_key(|&&reg| reg.order())?;

        self.registers_unused.remove(&reg);
        Some(reg)
    }

    fn spill(&mut self, location: usize, register: T) -> Action<T> {
        self.registers_in_use.remove(&register);
        let mem = self
            .spills_unused
            .iter()
            .next()
            .copied()
            .unwrap_or(self.spills_in_use.len());

        self.spills_in_use.insert(location, mem);

        Action::Spill(location, register)
    }

    fn unspill(&mut self, location: usize, into: T) -> Action<T> {
        let Some(mem) = self.spills_in_use.remove(&location) else {
            unreachable!()
        };

        self.allocate_register(into);
        self.spills_unused.insert(mem);

        Action::Unspill(location, into)
    }

    fn location_is_spilled(&self, location: usize) -> bool {
        self.spills_in_use.contains_key(&location)
    }

    fn deallocate_register(&mut self, r: T) {
        self.registers_in_use.remove(&r);
        self.registers_unused.insert(r);
    }

    fn allocate_register(&mut self, r: T) -> Action<T> {
        assert!(
            !self.registers_in_use.contains(&r),
            "register already in use"
        );
        self.registers_unused.remove(&r);
        self.registers_in_use.insert(r);

        Action::Allocate(r)
    }
}

#[derive(Debug)]
pub struct RegisterAllocator<T: Registers> {
    allocations: Box<[HashMap<usize, Action<T>>]>,
    active_leases: Box<[Box<[T]>]>,
    total_spills: usize,
}

impl<T: Registers> RegisterAllocator<T> {
    pub fn new(usage: &[Vec<bool>]) -> Self {
        let mut state = AllocatorState::<T>::new();

        let mut leases = HashMap::<usize, T>::new();
        let mut lease_expiry = HashMap::<usize, Vec<usize>>::new();

        let mut allocations = vec![HashMap::new(); usage.len()];
        let mut active_leases = Vec::with_capacity(usage.len());

        let registers_used_in_location = transpose(&usage);

        for (cur_location, registers) in registers_used_in_location.iter().enumerate() {
            let required_locations = registers
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(i, used)| if used { Some(i) } else { None })
                .collect::<Vec<_>>();

            let mut leases_to_acquire = required_locations
                .iter()
                .filter_map(|&i| {
                    if let Some(&reg) = leases.get(&i) {
                        allocations[cur_location].insert(i, Action::Use(reg));
                        None
                    } else {
                        Some(i)
                    }
                })
                .collect::<Vec<usize>>();

            leases_to_acquire.push(cur_location);

            for lease in lease_expiry.remove(&cur_location).unwrap_or(vec![]) {
                if let Some(register) = leases.remove(&lease) {
                    state.deallocate_register(register);
                }
            }

            for location in leases_to_acquire {
                let Some(expiry) = Self::get_lease_expiry(location, usage) else {
                    allocations[cur_location].insert(location, Action::None);
                    continue;
                };

                let destination = if let Some(reg) = state.get_unused_register() {
                    reg
                } else {
                    let spill_location =
                        Self::select_location_for_spilling(cur_location, &leases, usage);
                    let register = leases.remove(&spill_location).unwrap();
                    let spill = state.spill(spill_location, register);
                    allocations[cur_location].insert(spill_location, spill);
                    register
                };

                if state.location_is_spilled(location) {
                    let unspill = state.unspill(location, destination);
                    allocations[cur_location].insert(location, unspill);
                } else {
                    let allocate = state.allocate_register(destination);
                    allocations[cur_location].insert(location, allocate);
                }

                leases.insert(location, destination);
                lease_expiry
                    .entry(expiry)
                    .or_insert_with(Vec::new)
                    .push(location);
            }

            active_leases.push(
                leases
                    .values()
                    .copied()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }

        Self {
            allocations: allocations.into_boxed_slice(),
            total_spills: state.spills_unused.len() + state.spills_in_use.len(),
            active_leases: active_leases.into(),
        }
    }

    fn select_location_for_spilling(
        id: usize,
        leases: &HashMap<usize, T>,
        usage: &[Vec<bool>],
    ) -> usize {
        leases
            .keys()
            .map(|&location| {
                if let Some(next_use) = usage[location]
                    .iter()
                    .enumerate()
                    .skip(id)
                    .find(|(_, &used)| used)
                {
                    (location, next_use.0)
                } else {
                    unreachable!()
                }
            })
            .max_by_key(|(_, x)| *x)
            .unwrap()
            .0
    }

    fn get_lease_expiry(location: usize, usage: &[Vec<bool>]) -> Option<usize> {
        usage[location]
            .iter()
            .enumerate()
            .rfind(|(_, x)| **x)
            .map(|(exp, _)| exp + 1)
    }

    fn get_allocations(&self, location: usize) -> &HashMap<usize, Action<T>> {
        &self.allocations[location]
    }

    pub fn get_allocation_strategy(&self, cur_location: usize, location: usize) -> Action<T> {
        self.get_allocations(cur_location)
            .get(&location)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "no allocation strategy found for location {} in current location {}",
                    location, cur_location
                )
            })
    }

    pub fn get_spills(&self, cur_location: usize) -> Vec<(usize, T)> {
        self.get_allocations(cur_location)
            .values()
            .filter_map(|action| {
                if let Action::Spill(mem, reg) = action {
                    Some((*mem, *reg))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn spill_count(&self) -> usize {
        self.total_spills
    }

    pub fn get_active_leases(&self, location: usize) -> &[T] {
        &self.active_leases[location]
    }
}

macro_rules! registers {
    (
        $($name:ident($order:literal)),*
        $(,)?
    ) =>{
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum Register {
            $($name,)*
        }

        impl $crate::codegen::register_allocator::Registers for Register {
            fn count() -> usize {
                $crate::codegen::register_allocator::registers!(@count $($name),*)
            }

            fn all() -> Vec<Self> {
                vec![$(Self::$name),*]
            }

            fn order(&self) -> usize {
                match self {
                    $(Self::$name => $order,)*
                }
            }
        }
    };
    (@count $_:ident, $($extra:ident),*) => {
        1 + $crate::codegen::register_allocator::registers!(@count $($extra),*)
    };
    (@count $_:ident) => {
        1
    };
}

pub(crate) use registers;

use crate::util::transpose;
