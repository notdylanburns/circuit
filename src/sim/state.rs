use core::default;

#[derive(Copy, Clone, Default, PartialEq)]
pub(super) enum State {
    #[default]
    Z,
    L,
    H,
    E
}

impl State {
    pub const BITS: usize = 2;
    pub const STATE_COUNT: usize = 2usize.pow(Self::BITS as u32);
    pub const MAX_VAL: usize = Self::STATE_COUNT - 1;

    pub fn num<T: From<u8>>(&self) -> T {
        T::from(<u8>::from(self))
    }
}

impl core::ops::BitOr for &State {
    type Output = State;

    fn bitor(self, rhs: Self) -> Self::Output {
        State::from(u8::from(self) | u8::from(rhs))
    }
}

impl From<u8> for State {
    fn from(value: u8) -> Self {
        match value {
            0b00 => Self::Z,
            0b01 => Self::L,
            0b10 => Self::H,
            0b11 => Self::E,
            _ => unreachable!(),
        }
    }
}

impl From<&str> for State {
    fn from(value: &str) -> Self {
        match value {
            "Z"     => Self::Z,
            "0"|"L" => Self::L,
            "1"|"H" => Self::H,
            "E"     => Self::E,
            _       => unreachable!(),
        }
    }
}

impl From<&State> for u8 {
    fn from(value: &State) -> Self {
        match *value {
            State::Z => 0b00,
            State::L => 0b01,
            State::H => 0b10,
            State::E => 0b11,
            _ => todo!(),
        }
    }
}

impl From<&State> for &str {
    fn from(value: &State) -> Self {
        match *value {
            State::Z => "Z",
            State::L => "0",
            State::H => "1",
            State::E => "E",
            _ => todo!(),
        }
    }
}

impl core::fmt::Display for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", <&str>::from(self))
    }
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self, f)
    }
}

pub(super) enum StateArray<T> {
    One(usize, T),
    Many(usize, Vec<T>),
}

impl<T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> StateArray<T> {
    pub const COUNT: usize = 8 * core::mem::size_of::<T>() / State::BITS;

    const fn get_index(index: usize) -> (usize, usize) {
        (index / Self::COUNT, index % Self::COUNT)
    }

    const fn check_length<const N: usize>() {
        assert!(N == Self::COUNT);
    }

    const fn shift(index: usize) -> usize {
        index * State::BITS
    }

    const fn bit_mask(index: usize) -> usize {
        assert!(index < Self::COUNT);
        State::MAX_VAL << Self::shift(index)
    }

    pub fn permutations(count: usize) -> impl Iterator<Item = StateArray<T>> {
        (0..4usize.pow(count as u32))
            .map(move |i| StateArray::from_value(count, T::from(i)))
    }

    pub fn count(&self) -> usize {
        match self {
            Self::One(_, _) => 1,
            Self::Many(_, vs) => vs.len(),
        }
    }

    pub fn width(&self) -> usize {
        match self {
            Self::One(w, _) => *w,
            Self::Many(w, _) => *w,
        }
    }

    fn check_index(&self, index: usize) {
        let count = match self {
            Self::One(c, _) => c,
            Self::Many(c, _) => c,
        };

        if index > *count {
            panic!("Index out of bounds! {} > {}", index, *count)
        };
    }

    pub fn new(count: usize) -> Self {
        if count / Self::COUNT <= 1 {
            Self::One(count, T::from(0usize))
        } else {
            Self::Many(count, vec![T::from(0usize); count / Self::COUNT])
        }
    }

    pub fn preset(count: usize, state: &State) -> Self {
        let state_num = <u8>::from(state) as usize;
        let mut v = 0usize;
        for i in 0..count {
            v <<= State::BITS;
            v += state_num;
        }

        if count / Self::COUNT <= 1 {
            Self::One(count, T::from(v))
        } else {
            Self::Many(count, vec![T::from(v); count / Self::COUNT])
        }
    }

    pub fn error(count: usize) -> Self {
        if count / Self::COUNT <= 1 {
            Self::One(count, T::from(usize::MAX))
        } else {
            Self::Many(count, vec![T::from(usize::MAX); count / Self::COUNT])
        }
    }

    fn set_bits(value: &T, index: usize, state: &State) -> T {
        let mut cleared = (Into::<usize>::into(*value)) & (usize::MAX ^ Self::bit_mask(index));
        cleared |= (u8::from(state) as usize) << Self::shift(index);
        T::from(cleared)
    }

    pub fn set(&mut self, index: usize, state: &State) {
        self.check_index(index);

        let (value, idx) = match self {
            Self::One(_, v, ) => (v, index),
            Self::Many(_, vs) => {
                let (vec_idx, t_idx) = Self::get_index(index);
                (
                    vs.get_mut(vec_idx).expect("Index out of bounds!"),
                    t_idx
                )
            }
        };

        *value = Self::set_bits(value, idx, state)
    }

    pub fn set_range(&mut self, range: std::ops::Range<usize>, state: &[State]) {
        let rng_start = range.start;

        for index in range {
            self.set(index, &state[index - rng_start])
        };
    }

    fn get_bits(value: &T, index: usize) -> State {
        let mut bits = (Into::<usize>::into(*value)) & Self::bit_mask(index);
        State::from((bits >> Self::shift(index)) as u8)
    }

    pub fn get(&self, index: usize) -> State {
        self.check_index(index);

        let (value, idx) = match self {
            Self::One(_, v) => (v, index),
            Self::Many(_, vs) => {
                let (vec_idx, t_idx) = Self::get_index(index);
                (
                    vs.get(vec_idx).expect("Index out of bounds!"),
                    t_idx
                )
            }
        };

        Self::get_bits(value, index)
    }

    pub fn get_range(&self, range: std::ops::Range<usize>) -> Self {
        let range_start = range.start;
        let mut ret_val = Self::new(range.end - range_start);
        for index in range {
            ret_val.set(index - range_start, &self.get(index));
        };

        ret_val
    }

    pub fn get_value(&self) -> T {
        match self {
            Self::One(_, v) => *v,
            Self::Many(_, _) => panic!("get_value() can only be called on a single value StateArray"),
        }
    }

    pub fn from_value(elems: usize, value: T) -> Self {
        Self::One(elems, value)
    }

    pub fn merge(&self, other: &Self) -> Self {
        match self {
            Self::One(count, v) => {
                if let Self::One(ocount, ov) = other {
                    if count != ocount {
                        panic!("Cannot merge StateArrays with different element counts.");
                    }
                    Self::One(*count, *v | *ov)
                } else {
                    panic!("Cannot merge StateArrays of different types.");
                }
            }
            Self::Many(count, vs) => {
                if let Self::Many(ocount, ovs) = other {
                    if count != ocount {
                        panic!("Cannot merge StateArrays with different element counts.");
                    }

                    Self::Many(
                        *count,
                        vs.iter().zip(ovs.iter())
                            .map(|(state1, state2)| *state1 | *state2)
                            .collect()
                    )
                } else {
                    panic!("Cannot perform bitwise or between StateArrays of different types.");
                }
            }
        }
    }

    pub fn is_one(&self) -> bool {
        matches!(self, Self::One(..))
    }

    pub fn is_many(&self) -> bool {
        matches!(self, Self::Many(..))
    }

    pub fn merge_all(states: &[Self]) -> Self {
        if states.is_empty() {
            return Self::new(0);
        }

        let width = states[0].width();
        states.iter()
            .fold(
                Self::new(width),
                |state1, state2| &state1 | state2
            )
    }

    pub fn negate(&self) -> Self {
        self.iter()
            .map(|s| match s {
                State::L => State::H,
                State::H => State::L,
                _ => s,
            })
            .collect()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            index: 0,
            data: self,
        }
    }

    pub fn as_vec(&self) -> Vec<State> {
        self.iter().collect()
    }
}

impl<T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> core::ops::BitOr<&StateArray<T>> for &StateArray<T> {
    type Output = StateArray<T>;

    fn bitor(self, rhs: &StateArray<T>) -> Self::Output {
        self.merge(rhs)
    }
}

impl<T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T> + std::cmp::PartialEq> PartialEq for StateArray<T> {
    fn eq(&self, other: &Self) -> bool {
        self.count() == other.count() &&
        self.width() == other.width() &&
        match self {
            Self::One(_, v) => other.is_one() && *v == other.get_value(),
            Self::Many(_, vs) => match other {
                Self::Many(_, ovs) => vs.iter().zip(ovs.iter()).fold(true, |acc, (v, ov)| acc && v == ov),
                _ => false
            }
        }
    }
}

impl<T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr> Clone for StateArray<T> {
    fn clone(&self) -> Self {
        match self {
            Self::One(elems, v) => Self::One(*elems, *v),
            Self::Many(elems, vs) => Self::Many(*elems, vs.clone()),
        }
    }
}

impl<T, U> From<U> for StateArray<T>
where 
    T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>,
    U: AsRef<[State]>
{
    fn from(value: U) -> Self {
        let val = value.as_ref();
        let len = val.len();
        let mut ret = Self::new(len);

        ret.set_range(0..len, val);

        ret
    }
}

impl<T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> std::fmt::Debug for StateArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let contents = self.iter()
            .map(|v| <&str>::from(&v))
            .collect::<Vec<&str>>()
            .join(", ");

        write!(f, "[{contents}]")
    }
}

impl <T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> std::iter::FromIterator<State> for StateArray<T> {
    fn from_iter<I: IntoIterator<Item = State>>(iter: I) -> Self {
        let mut sa = state_array!();
        iter.into_iter()
            .enumerate()
            .for_each(|(idx, s)| sa.set(idx, &s));

        sa
    }
}

pub(super) struct Iter<'a, T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> {
    index: usize,
    data: &'a StateArray<T>,
}

impl<'a, T: Sized + Copy + Into<usize> + From<usize> + core::ops::BitOr<Output = T>> Iterator for Iter<'a, T> {
    type Item = State;

    fn next(&mut self) -> Option<Self::Item> {
        let len = match *self.data {
            StateArray::One(c, _) => c,
            StateArray::Many(c, _) => c,
        };

        let retval = if self.index >= len {
            None
        } else {
            Some(self.data.get(self.index))
        };

        self.index += 1;

        retval
    }
}

macro_rules! state_array {
    [$($tt:tt)*] => {
        $crate::sim::state::StateArray::from(&[$($tt)*])
    };
}

pub(super) use state_array;


pub(super) struct StateAllocator {
    states: Vec<State>
}

impl StateAllocator {
    pub fn new() -> Self {
        Self {
            states: vec![],
        }
    }

    pub fn allocate(&mut self, count: usize) -> std::ops::Range<usize> {
        let previous_len = self.states.len();
        self.states.extend((0..count).map(|_| State::default()));

        previous_len..self.states.len()
    }

    pub fn read(&self, range: &std::ops::Range<usize>) -> &[State] {
        self.states.get(range.clone()).expect("StateAllocator::read(): Range out of bounds")
    }

    pub fn write(&mut self, range: &std::ops::Range<usize>, values: &[State]) {
        if range.len() != values.len() {
            panic!("StateAllocator::write(): Too many values for range")
        };

        self.states
            .get_mut(range.clone())
            .expect("StateAllocator::write(): Range out of bounds")
            .iter_mut()
            .zip(values)
            .for_each(|(d, s)| *d = *s)
    }
}

trait Connection {
    fn read<'a>(&self, sa: &'a StateAllocator) -> &'a [State];
    fn write(&self, sa: &mut StateAllocator, vs: &[State]);
}


#[test]
fn test() {
    struct Pin {
        state_range: std::ops::Range<usize>,
    };

    impl Connection for Pin {
        fn read<'a>(&self, sa: &'a StateAllocator) -> &'a [State] {
            sa.read(&self.state_range)
        }

        fn write(&self, sa: &mut StateAllocator, vs: &[State]) {
            sa.write(&self.state_range, vs)
        }
    };
}