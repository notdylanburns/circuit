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
        index: Option<usize>,
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
struct EndpointBuilder {
    t: EndpointBuilderType,
    pin: Option<PinId>,
    dependency_name: Option<IdentId>,
    index: Option<usize>,
    range: Option<ConnectionRange>,
}

impl EndpointBuilder {
    fn pin_id(&mut self, pin: PinId) -> &mut Self {
        self.t = EndpointBuilderType::Pin;
        self.pin.replace(pin);
        self
    }

    fn name(&mut self, name: IdentId) -> &mut Self {
        self.t = EndpointBuilderType::Dependency;
        self.dependency_name.replace(name);
        self.index = None;
        self
    }

    fn array_index(&mut self, array_name: IdentId, index: usize) -> &mut Self {
        self.t = EndpointBuilderType::Dependency;
        self.dependency_name.replace(array_name);
        self.index = Some(index);
        self
    }

    fn full(&mut self) -> &mut Self {
        self.range.replace(ConnectionRange::FULL);
        self
    }

    fn single(&mut self, index: usize) -> &mut Self {
        self.range.replace(ConnectionRange::Single(index));
        self
    }

    fn range_unbound_end(&mut self, start: usize) -> &mut Self {
        self.range
            .replace(ConnectionRange::Range(Some(start), None));
        self
    }

    fn range_unbound_start(&mut self, end: usize) -> &mut Self {
        self.range.replace(ConnectionRange::Range(None, Some(end)));
        self
    }

    fn range(&mut self, start: usize, end: usize) -> &mut Self {
        self.range
            .replace(ConnectionRange::Range(Some(start), Some(end)));
        self
    }

    fn build(self) -> ConnectionEndpointBuilder {
        ConnectionEndpointBuilder::Dependency {
            dependency_name: self.dependency_name.unwrap_or_else(|| {
                unreachable!("dependency_name field not set for DependencyEndpointBuilder")
            }),
            index: self.index,
            range: self.range.unwrap_or_else(|| {
                unreachable!("range field not set for DependencyEndpointBuilder")
            }),
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

    fn new_unidirectional() -> Self {
        Self::new().unidirectional()
    }

    fn new_bidirectional() -> Self {
        Self::new().bidirectional()
    }

    fn unidirectional(mut self) -> Self {
        self.connection_type.replace(ConnectionType::Unidirectional);
        self
    }

    fn bidirectional(mut self) -> Self {
        self.connection_type.replace(ConnectionType::Bidirectional);
        self
    }

    fn source(&mut self) -> &mut EndpointBuilder {
        &mut self.source
    }

    fn destination(&mut self) -> &mut EndpointBuilder {
        &mut self.source
    }
}

#[derive(Default)]
struct CircBuilder {
    dependencies: HashMap<IdentId, (CircId, usize)>,
    pins: OrderedMap<IdentId, Pin>,
    connections: Vec<ConnectionBuilder>,
}

impl CircBuilder {
    fn dependency(&mut self, name: IdentId, circ_id: CircId, count: Option<usize>) -> &mut Self {
        self.dependencies.insert(name, (circ_id, count.unwrap_or(1)));
        self
    }

    fn pin(&mut self, name: IdentId, pin: Pin) -> &mut Self {
        self.pins.insert(name, pin);
        self
    }

    fn connection(&mut self) -> &mut ConnectionBuilder {
        self.connections.push(ConnectionBuilder::new());
        self.connections.last_mut().unwrap()
    }

    fn build(self) -> Circ {
        let mut dependency_array = self.dependencies.into_iter().collect::<Vec<_>>();
        dependency_array.sort_by_key(|(_, (v, _))| *v);
        let dependencies = dependency_array
            .into_iter()
            .map(|(name, (circ_id, count))|
                 std::iter::repeat_n(circ_id, count)
                    .enumerate()
                    .zip(std::iter::repeat(name))
                    .map(|((i, circ_id), name)| (name, i, circ_id))
            ).flatten();

        todo!()
    }
}
