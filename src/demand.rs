/// Identifier of a node in the toy city network.
pub type NodeId = usize;

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
        let demand = Demand::new(1, 2, 100);
        assert_eq!(demand.origin, 1);
        assert_eq!(demand.destination, 2);
        assert_eq!(demand.amount, 100);
    }
}
