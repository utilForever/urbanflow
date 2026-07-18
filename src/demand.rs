pub type NodeId = usize;

/// A minimal origin-destination demand entry.
///
/// Represents the flow pressure from `origin` to `destination` as a scalar
/// `amount`. This is intentionally small for Sprint 1: it only references the
/// two node ids and stores the demand amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demand {
    pub origin: NodeId,
    pub destination: NodeId,
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
