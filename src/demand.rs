use crate::world::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demand {
    /// Node the demand originates from.
    pub origin: NodeId,
    /// Node the demand is headed to.
    pub destination: NodeId,
    /// Amount of demand flowing from `origin` to `destination`.
    pub amount: u32,
}

impl Demand {
    pub fn new(origin: NodeId, destination: NodeId, amount: u32) -> Self {
        Self {
            origin,
            destination,
            amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demand_creation() {
        let demand = Demand::new(NodeId(1), NodeId(2), 100);
        assert_eq!(demand.origin, NodeId(1));
        assert_eq!(demand.destination, NodeId(2));
        assert_eq!(demand.amount, 100);
    }
}
