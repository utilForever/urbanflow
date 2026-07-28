use urbanflow::demand::Demand;
use urbanflow::network::ConnectivityIndex;
use urbanflow::world::{EdgeKind, Network, NodeId};

/// Builds the toy network `0 -(Road)-> 1` and `2 -(Rail)-> 3`, which forms two
/// separate components until something bridges them.
fn disconnected_network() -> Network {
    let mut network = Network::new();
    network
        .add_edge(NodeId(0), NodeId(1), EdgeKind::Road)
        .unwrap();
    network
        .add_edge(NodeId(2), NodeId(3), EdgeKind::Rail)
        .unwrap();
    network
}

/// A demand can only be served once the network carries a path from its origin
/// to its destination. Adding a single bridging edge is enough to flip the
/// outcome, which guards the core reachability behavior.
#[test]
fn connectivity_changes_demand_handling() {
    let demand = Demand::new(NodeId(0), NodeId(3), 10);

    // Disconnected: nothing links the 0-1 component to the 2-3 one.
    let mut network = disconnected_network();
    assert!(!ConnectivityIndex::from_network(&network).can_serve(&demand));

    // Connected: bridging 1 -> 2 completes the path 0 -> 1 -> 2 -> 3.
    network
        .add_edge(NodeId(1), NodeId(2), EdgeKind::Road)
        .unwrap();
    assert!(ConnectivityIndex::from_network(&network).can_serve(&demand));
}

/// Links are one-way, so a return trip is its own demand served by its own
/// edges. Building the forward path alone must not serve the reverse flow.
#[test]
fn return_trip_needs_its_own_edges() {
    let mut network = disconnected_network();
    network
        .add_edge(NodeId(1), NodeId(2), EdgeKind::Road)
        .unwrap();

    let outbound = Demand::new(NodeId(0), NodeId(3), 10);
    let inbound = Demand::new(NodeId(3), NodeId(0), 10);

    let index = ConnectivityIndex::from_network(&network);
    assert!(index.can_serve(&outbound));
    assert!(!index.can_serve(&inbound));

    // Laying the mirrored edges makes the return trip servable too.
    network
        .add_edge(NodeId(3), NodeId(2), EdgeKind::Rail)
        .unwrap();
    network
        .add_edge(NodeId(2), NodeId(1), EdgeKind::Road)
        .unwrap();
    network
        .add_edge(NodeId(1), NodeId(0), EdgeKind::Road)
        .unwrap();

    assert!(ConnectivityIndex::from_network(&network).can_serve(&inbound));
}
