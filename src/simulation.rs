use crate::demand::Demand;
use crate::metrics::Metrics;
use crate::network::ConnectivityIndex;
use crate::world::Network;

fn allocate(network: &Network, demands: &[Demand]) -> (u64, Vec<u64>) {
    let connectivity = ConnectivityIndex::from_network(network);
    let mut served = 0;
    let mut loads = vec![0; network.edges().len()];

    for demand in demands {
        let Some(path) = connectivity.path(demand.origin, demand.destination) else {
            continue;
        };
        let amount = path
            .iter()
            .map(|edge| network.edges()[edge.0].kind.capacity())
            .min()
            .unwrap_or(demand.amount)
            .min(demand.amount);

        served += u64::from(amount);

        for edge in path {
            loads[edge.0] += u64::from(amount);
        }
    }

    (served, loads)
}

pub(crate) fn tick(network: &Network, demands: &[Demand]) -> Metrics {
    let total_demand = demands
        .iter()
        .try_fold(0_u64, |total, demand| {
            total.checked_add(u64::from(demand.amount))
        })
        .expect("total demand exceeds metric capacity");
    let (served, _) = allocate(network, demands);

    Metrics::new(
        served,
        total_demand - served,
        0.0,
        network.edges().len() as f64,
    )
}

#[cfg(test)]
mod tests {
    use super::{allocate, tick};
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

    #[test]
    fn allocation_stops_at_path_bottleneck_and_records_loads() {
        let mut network = Network::new();
        network
            .add_edge(NodeId(0), NodeId(1), EdgeKind::Road)
            .unwrap();
        network
            .add_edge(NodeId(1), NodeId(2), EdgeKind::Rail)
            .unwrap();

        assert_eq!(
            allocate(&network, &[Demand::new(NodeId(0), NodeId(2), 15)]),
            (10, vec![10, 10])
        );
    }
}
