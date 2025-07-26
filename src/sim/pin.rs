use core::slice::SliceIndex;
use std::{ops::Range, process::Output};

use crate::ref_util::{Ref, MakeRef};
use super::StateArray;
use super::state::State;

#[derive(Clone, PartialEq)]
pub(super) enum PinDirection {
    Input,
    Output,
    Any,
}

pub(super) struct SymbolicPin {
    width: usize,
    initial: Vec<State>,
    is_tri_state: bool,
    direction: PinDirection,
}

#[derive(Clone)]
pub(super) struct Pin {
    width: usize,
    direction: PinDirection,
    dirty: bool,
    states: Vec<StateArray>,
}

impl Pin {
    pub fn new(width: usize, direction: PinDirection) -> Self {
        Self {
            width,
            direction,
            dirty: false,
            states: vec![],
        }
    }

    fn resolve_state(&self, except: usize) -> StateArray {
        let resolved = self.states
            .iter()
            .enumerate()
            .filter_map(|(idx, v)|
                if idx == except {
                    None
                } else {
                    Some(v)
                }
            )
            .fold(
                StateArray::new(self.width),
                |state1, state2| &state1 | state2
            );

        resolved
    }

    fn read(&self, config: &PinConnectionConfig) -> StateArray {
        self.resolve_state(config.state_index)
            .get_range(config.range())
    }

    fn write(&mut self, config: &PinConnectionConfig, state: &[State]) {
        self.states[config.state_index]
            .set_range(config.range(), state)
    }

    fn connect(&mut self, start_index: usize, end_index: usize) -> PinConnectionConfig {
        let state_index = self.states.len();
        self.states.push(StateArray::new(self.width));

        PinConnectionConfig {
            start_index,
            end_index,
            state_index
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn direction(&self) -> PinDirection {
        self.direction.clone()
    }
}

struct PinConnectionConfig {
    start_index: usize,
    end_index: usize,
    state_index: usize,
}

impl PinConnectionConfig {
    fn range(&self) -> std::ops::Range<usize> {
        if self.start_index == self.end_index {
            self.start_index..(self.end_index + 1)
        } else {
            self.start_index..self.end_index
        }
    }
}

pub(super) struct PinConnection {
    config: PinConnectionConfig,
    pin: Ref<Pin>,
}

impl PinConnection {
    pub fn new(mut pin: Ref<Pin>, start_index: usize, end_index: usize) -> Self {
        assert!(start_index <= end_index);
        let config = pin.with_mut(|mut p| {
            assert!(p.width() > end_index);
            p.connect(start_index, end_index)
        });

        Self {
            pin,
            config,
        }
    }

    pub fn read(&self) -> StateArray {
        self.pin
            .with(|p| p.read(&self.config))
    }

    pub fn write(&mut self, state: &[State]) {
        self.pin
            .with_mut(|mut p| p.write(&self.config, state))
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.config.end_index - self.config.start_index + 1
    }
}

