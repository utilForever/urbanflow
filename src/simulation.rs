use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::network::ConnectivityIndex;
use crate::world::Network;

pub(crate) fn tick(network: &Network, demands: &[Demand]) -> Metrics {
    let total_demand = demands
        .iter()
        .try_fold(0_u64, |total, demand| {
            total.checked_add(u64::from(demand.amount))
        })
        .expect("total demand exceeds metric capacity");
    let connectivity = ConnectivityIndex::from_network(network);
    let served = demands
        .iter()
        .filter(|demand| connectivity.can_serve(demand))
        .map(|demand| u64::from(demand.amount))
        .sum();

    Metrics::new(
        served,
        total_demand - served,
        0.0,
        network.edges().len() as f64,
    )
}

#[cfg(test)]
mod tests {
    use super::tick;
    use crate::demand::Demand;
    use crate::metrics::Metrics;
    use crate::world::{EdgeKind, Network, NodeId};

    #[test]
    fn tick_deterministically_aggregates_demand_metrics() {
        let mut network = Network::new();
        network
            .add_edge(NodeId(0), NodeId(1), EdgeKind::Road)
            .unwrap();
        network
            .add_edge(NodeId(2), NodeId(3), EdgeKind::Rail)
            .unwrap();
        let demands = [
            Demand::new(NodeId(0), NodeId(1), 7),
            Demand::new(NodeId(1), NodeId(0), 5),
            Demand::new(NodeId(2), NodeId(3), 11),
        ];
        let expected = Metrics::new(18, 5, 0.0, 2.0);

        assert_eq!(tick(&network, &demands), expected);
        assert_eq!(tick(&network, &demands), expected);
    }
}
