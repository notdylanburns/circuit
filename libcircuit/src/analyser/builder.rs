use super::{
    Circ, CircId, Connection, ConnectionEndpoint, ConnectionRange, ConnectionType, IdentId, Pin,
    PinId,
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

#[derive(Default)]
enum EndpointBuilderType {
    #[default]
    None,
    Pin,
    Dependency,
}

#[derive(Default)]
pub(super) struct EndpointBuilder {
    t: EndpointBuilderType,
    pin: Option<PinId>,
    dependency_name: Option<IdentId>,
    index: Option<usize>,
    range: Option<ConnectionRange>,
}

impl EndpointBuilder {
    pub(super) fn pin_id(&mut self, pin: PinId) -> &mut Self {
        self.t = EndpointBuilderType::Pin;
        self.pin.replace(pin);
        self
    }

    pub(super) fn name(&mut self, name: IdentId) -> &mut Self {
        self.t = EndpointBuilderType::Dependency;
        self.dependency_name.replace(name);
        self.index = None;
        self
    }

    pub(super) fn array_index(&mut self, array_name: IdentId, index: usize) -> &mut Self {
        self.t = EndpointBuilderType::Dependency;
        self.dependency_name.replace(array_name);
        self.index = Some(index);
        self
    }

    pub(super) fn full(&mut self) -> &mut Self {
        self.range.replace(ConnectionRange::FULL);
        self
    }

    pub(super) fn single(&mut self, index: usize) -> &mut Self {
        self.range.replace(ConnectionRange::Single(index));
        self
    }

    pub(super) fn range_unbound_end(&mut self, start: usize) -> &mut Self {
        self.range
            .replace(ConnectionRange::Range(Some(start), None));
        self
    }

    pub(super) fn range_unbound_start(&mut self, end: usize) -> &mut Self {
        self.range.replace(ConnectionRange::Range(None, Some(end)));
        self
    }

    pub(super) fn range(&mut self, start: Option<usize>, end: Option<usize>) -> &mut Self {
        self.range.replace(ConnectionRange::Range(start, end));
        self
    }

    fn build(self) -> ConnectionEndpointBuilder {
        match self.t {
            EndpointBuilderType::None => unreachable!("EndpointBuilderType::None is not allowed"),
            EndpointBuilderType::Pin => ConnectionEndpointBuilder::Pin {
                pin_id: self
                    .pin
                    .unwrap_or_else(|| unreachable!("pin field not set for PinEndpointBuilder")),
                range: self
                    .range
                    .unwrap_or_else(|| unreachable!("range field not set for PinEndpointBuilder")),
            },
            EndpointBuilderType::Dependency => ConnectionEndpointBuilder::Dependency {
                dependency_name: self.dependency_name.unwrap_or_else(|| {
                    unreachable!("dependency_name field not set for DependencyEndpointBuilder")
                }),
                index: self.index.unwrap_or(0),
                pin_id: self.pin.unwrap_or_else(|| {
                    unreachable!("pin field not set for DependencyEndpointBuilder")
                }),
                range: self.range.unwrap_or_else(|| {
                    unreachable!("range field not set for DependencyEndpointBuilder")
                }),
            },
        }
    }
}

#[derive(Default)]
struct ConnectionBuilder {
    source: EndpointBuilder,
    dest: EndpointBuilder,
    connection_type: Option<ConnectionType>,
}

impl ConnectionBuilder {
    fn new() -> Self {
        Self::default()
    }

    pub(super) fn unidirectional(&mut self) -> &mut Self {
        self.connection_type.replace(ConnectionType::Unidirectional);
        self
    }

    pub(super) fn bidirectional(&mut self) -> &mut Self {
        self.connection_type.replace(ConnectionType::Bidirectional);
        self
    }

    pub(super) fn source(&mut self) -> &mut EndpointBuilder {
        &mut self.source
    }

    pub(super) fn destination(&mut self) -> &mut EndpointBuilder {
        &mut self.source
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
        }
    }
}

#[derive(Default)]
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
        self.pins.insert(name, pin);
        self
    }

    pub(super) fn get_pin(&self, name: IdentId) -> Option<(PinId, Pin)> {
        self.pins
            .get_index(&name)
            .map(|id| (id, *self.pins.get(&name).unwrap()))
    }

    pub(super) fn connection(&mut self) -> &mut ConnectionBuilder {
        self.connections.push(ConnectionBuilder::new());
        self.connections.last_mut().unwrap()
    }

    pub(super) fn build(self) -> Circ {
        let mut dependency_array = self.dependencies.into_iter().collect::<Vec<_>>();
        dependency_array.sort_by_key(|(_, (v, _))| *v);
        let dependencies = dependency_array
            .into_iter()
            .map(|(name, (circ_id, count))| {
                std::iter::repeat_n(circ_id, count.unwrap_or(1))
                    .enumerate()
                    .zip(std::iter::repeat(name))
                    .map(|((i, circ_id), name)| (name, i, circ_id))
            })
            .flatten();

        let dependency_start_indicies = dependencies
            .clone()
            .filter_map(|(name, i, _)| if i == 0 { Some((name, i)) } else { None })
            .collect::<HashMap<_, _>>();

        let dependencies = dependencies
            .map(|(_, _, circ_id)| circ_id)
            .collect::<Vec<_>>();

        let connections = self
            .connections
            .into_iter()
            .map(|c| c.build(&dependency_start_indicies))
            .collect::<Vec<_>>();

        Circ {
            dependencies: Rc::from(&dependencies[..]),
            pins: Rc::new(self.pins),
            connections: Rc::from(&connections[..]),
        }
    }
}
