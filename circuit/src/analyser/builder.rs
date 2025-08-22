use super::{
    Circ, CircId, Connection, ConnectionEndpoint, ConnectionRange, ConnectionType, IdentId,
    ModuleCirc, Pin, PinDirection, PinExprType, PinId,
};
use crate::util::OrderedMap;

use std::{collections::HashMap, rc::Rc};

enum ConnectionEndpointBuilder {
    Pin {
        pin_id: PinId,
        range: ConnectionRange,
    },
    Dependency {
        dependency_name: IdentId,
        index: usize,
        pin_id: PinId,
        range: ConnectionRange,
    },
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum EndpointBuilderType {
    #[default]
    None,
    Pin,
    Dependency,
}

#[derive(Debug, Default)]
pub(super) struct EndpointBuilder {
    t: EndpointBuilderType,
    pin: Option<PinId>,
    dependency_name: Option<IdentId>,
    index: Option<usize>,
    range: Option<ConnectionRange>,
}

impl EndpointBuilder {
    pub(super) fn pin_id(&mut self, pin: PinId) -> &mut Self {
        if self.t == EndpointBuilderType::None {
            self.t = EndpointBuilderType::Pin;
        }
        self.pin.replace(pin);
        self
    }

    pub(super) fn dependency(&mut self, array_name: IdentId, index: Option<usize>) -> &mut Self {
        self.t = EndpointBuilderType::Dependency;
        self.dependency_name.replace(array_name);
        self.index = index;
        self
    }

    pub(super) fn range(&mut self, start: usize, end: usize) -> &mut Self {
        self.range.replace(ConnectionRange { start, end });
        self
    }

    fn build(self) -> ConnectionEndpointBuilder {
        match self.t {
            EndpointBuilderType::None => unreachable!("EndpointBuilderType::None is not allowed"),
            EndpointBuilderType::Pin => ConnectionEndpointBuilder::Pin {
                pin_id: self
                    .pin
                    .unwrap_or_else(|| unreachable!("pin field not set for PinEndpointBuilder")),
                range: self.range.unwrap_or_else(|| unreachable!("range not set")),
            },
            EndpointBuilderType::Dependency => ConnectionEndpointBuilder::Dependency {
                dependency_name: self.dependency_name.unwrap_or_else(|| {
                    unreachable!("dependency_name field not set for DependencyEndpointBuilder")
                }),
                index: self.index.unwrap_or(0),
                pin_id: self.pin.unwrap_or_else(|| {
                    unreachable!("pin field not set for DependencyEndpointBuilder")
                }),
                range: self.range.unwrap_or_else(|| unreachable!("range not set")),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnType {
    PinToPin,
    PinToDep,
    DepToPin,
    DepToDep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CannotConnectReason {
    Unknown,
    WidthMismatch(usize, usize),
    DirectionMismatch {
        conn_type: ConnType,
        lhs: PinDirection,
        rhs: PinDirection,
    },
    InvalidEndpoint {
        lhs: bool,
        rhs: bool,
    },
}

#[derive(Debug, Default)]
pub(super) struct ConnectionBuilder {
    width: Option<usize>,
    source: EndpointBuilder,
    dest: EndpointBuilder,
    connection_type: Option<ConnectionType>,
}

impl ConnectionBuilder {
    fn new() -> Self {
        Self::default()
    }

    pub(super) fn unidirectional(mut self) -> Self {
        self.connection_type.replace(ConnectionType::Unidirectional);
        self
    }

    pub(super) fn bidirectional(mut self) -> Self {
        self.connection_type.replace(ConnectionType::Bidirectional);
        self
    }

    pub(super) fn connect(
        &mut self,
        lhs: PinExprType,
        rhs: PinExprType,
    ) -> Result<(), CannotConnectReason> {
        if lhs.is_unknown() || rhs.is_unknown() {
            return Err(CannotConnectReason::Unknown);
        };

        let lhs_valid = lhs.is_valid_endpoint();
        let rhs_valid = rhs.is_valid_endpoint();

        if !lhs_valid || !rhs_valid {
            return Err(CannotConnectReason::InvalidEndpoint {
                lhs: lhs_valid,
                rhs: rhs_valid,
            });
        };

        let conn_type = match (lhs.is_dependency_pin(), rhs.is_dependency_pin()) {
            (false, false) => ConnType::PinToPin,
            (false, true) => ConnType::PinToDep,
            (true, false) => ConnType::DepToPin,
            (true, true) => ConnType::DepToDep,
        };

        let lhs_dir = lhs.direction();
        let rhs_dir = rhs.direction();

        match (conn_type, lhs_dir, rhs_dir) {
            (ConnType::PinToPin, PinDirection::Input, PinDirection::Output) => (),
            (ConnType::DepToDep, PinDirection::Output, PinDirection::Input) => (),
            (ConnType::PinToDep, PinDirection::Input, PinDirection::Input) => (),
            (ConnType::DepToPin, PinDirection::Output, PinDirection::Output) => (),
            (_, PinDirection::Transput, _) | (_, _, PinDirection::Transput) => (),
            _ => {
                return Err(CannotConnectReason::DirectionMismatch {
                    conn_type,
                    lhs: lhs_dir,
                    rhs: rhs_dir,
                })
            }
        };

        let lhs_width = lhs.width();
        let rhs_width = rhs.width();

        if lhs_width != rhs_width {
            return Err(CannotConnectReason::WidthMismatch(lhs_width, rhs_width));
        };

        self.width = Some(lhs_width);
        self.set_source(lhs).set_destination(rhs);

        Ok(())
    }

    fn get_endpoint(endpoint: PinExprType) -> EndpointBuilder {
        let mut builder = EndpointBuilder::default();
        match endpoint {
            PinExprType::Unknown
            | PinExprType::Index(_)
            | PinExprType::Range(_, _)
            | PinExprType::DependencyArray { .. }
            | PinExprType::Dependency { .. } => unreachable!(),
            PinExprType::Pin { pin_id, pin, .. } => builder.pin_id(pin_id).range(0, pin.width),
            PinExprType::PinRange {
                pin_id,
                pin,
                start,
                end,
                ..
            } => builder
                .pin_id(pin_id)
                .range(start.unwrap_or(0), end.unwrap_or(pin.width)),
            PinExprType::DependencyPin {
                name,
                index,
                pin_id,
                pin,
                ..
            } => builder
                .dependency(name, index)
                .pin_id(pin_id)
                .range(0, pin.width),
            PinExprType::DependencyPinRange {
                name,
                index,
                pin_id,
                pin,
                start,
                end,
                ..
            } => builder
                .dependency(name, index)
                .pin_id(pin_id)
                .range(start.unwrap_or(0), end.unwrap_or(pin.width)),
        };

        builder
    }

    fn set_source(&mut self, endpoint: PinExprType) -> &mut Self {
        self.source = Self::get_endpoint(endpoint);
        self
    }

    fn set_destination(&mut self, endpoint: PinExprType) -> &mut Self {
        self.dest = Self::get_endpoint(endpoint);
        self
    }

    fn build_endpoint(
        endpoint: ConnectionEndpointBuilder,
        dependency_starts: &HashMap<IdentId, usize>,
    ) -> ConnectionEndpoint {
        match endpoint {
            ConnectionEndpointBuilder::Pin { pin_id, range } => {
                ConnectionEndpoint::Pin { pin_id, range }
            }
            ConnectionEndpointBuilder::Dependency {
                dependency_name,
                index,
                pin_id,
                range,
            } => {
                let dependency_id = dependency_starts
                    .get(&dependency_name)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "Dependency {} not found in dependency_starts",
                            dependency_name
                        )
                    })
                    + index;
                ConnectionEndpoint::Dependency {
                    dependency_id,
                    pin_id,
                    range,
                }
            }
        }
    }

    fn build(self, dependency_starts: &HashMap<IdentId, usize>) -> Connection {
        let source = Self::build_endpoint(self.source.build(), dependency_starts);
        let dest = Self::build_endpoint(self.dest.build(), dependency_starts);

        Connection {
            source,
            dest,
            connection_type: self
                .connection_type
                .unwrap_or_else(|| unreachable!("ConnectionType not set")),
            width: self.width.unwrap_or_else(|| unreachable!("width not set")),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct CircBuilder {
    dependencies: HashMap<IdentId, (CircId, Option<usize>)>,
    pins: OrderedMap<IdentId, Pin>,
    connections: Vec<ConnectionBuilder>,
}

impl CircBuilder {
    pub(super) fn dependency(
        &mut self,
        name: IdentId,
        circ_id: CircId,
        count: Option<usize>,
    ) -> &mut Self {
        self.dependencies.insert(name, (circ_id, count));
        self
    }

    pub(super) fn get_dependency(&self, name: IdentId) -> Option<(CircId, Option<usize>)> {
        self.dependencies.get(&name).copied()
    }

    pub(super) fn pin(&mut self, name: IdentId, pin: Pin) -> &mut Self {
        let _ = self.pins.insert(name, pin);
        self
    }

    pub(super) fn get_pin(&self, name: IdentId) -> Option<(PinId, Pin)> {
        self.pins
            .get_index(&name)
            .map(|id| (id, *self.pins.get(&name).unwrap()))
    }

    pub(super) fn add_connection(&mut self, connection: ConnectionBuilder) -> &mut Self {
        self.connections.push(connection);
        self
    }

    pub(super) fn build(self) -> Circ {
        let mut dependency_array = self.dependencies.into_iter().collect::<Vec<_>>();
        dependency_array.sort_by_key(|(_, (v, _))| *v);
        let dependencies = dependency_array
            .into_iter()
            .flat_map(|(name, (circ_id, count))| {
                std::iter::repeat_n(circ_id, count.unwrap_or(1))
                    .enumerate()
                    .zip(std::iter::repeat(name))
                    .map(|((i, circ_id), name)| (name, i, circ_id))
            });

        let dependency_start_indicies = dependencies
            .clone()
            .enumerate()
            .filter_map(|(start, (name, i, _))| if i == 0 { Some((name, start)) } else { None })
            .collect::<HashMap<_, _>>();

        let dependencies = dependencies
            .map(|(_, _, circ_id)| circ_id)
            .collect::<Vec<_>>();

        let connections = self
            .connections
            .into_iter()
            .filter(|c| c.width.is_some())
            .map(|c| c.build(&dependency_start_indicies))
            .collect::<Vec<_>>();

        ModuleCirc {
            dependencies: Rc::from(&dependencies[..]),
            pins: Rc::new(self.pins),
            connections: Rc::from(&connections[..]),
        }
        .into()
    }
}
