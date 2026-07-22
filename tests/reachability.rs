use urbanflow::demand::Demand;
use urbanflow::network::Network;

/// The same demand should be served only once its origin and destination
/// belong to the same connected component. Adding a single bridging edge is
/// enough to flip the outcome, which guards the core reachability behavior.
#[test]
fn connectivity_changes_demand_handling() {
    let demand = Demand::new(0, 3, 10);

    // Disconnected: 0-1 and 2-3 form two separate components, so the demand
    // from 0 to 3 cannot be served.
    let mut disconnected = Network::new();
    disconnected.add_edge(0, 1);
    disconnected.add_edge(2, 3);
    assert!(!disconnected.can_serve(&demand));

    // Connected: bridging 1-2 links everything into 0-1-2-3, so the same
    // demand can now be served.
    let mut connected = disconnected.clone();
    connected.add_edge(1, 2);
    assert!(connected.can_serve(&demand));
}
