/// A single snapshot of core network behavior for Sprint 1.
///
/// Captures the first useful output measures: how much demand was served
/// versus left unserved, plus aggregate congestion and cost. These are the
/// raw counters that Sprint 2 will later turn into RL rewards; this type
/// intentionally stays a flat snapshot, with no time-series or reward logic.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Metrics {
    /// Total demand amount that was successfully served.
    pub served_demand: u32,
    /// Total demand amount that could not be served.
    pub unserved_demand: u32,
    /// Aggregate congestion level of the network.
    pub congestion: f64,
    /// Aggregate network cost.
    pub cost: f64,
}

impl Metrics {
    pub fn new(served_demand: u32, unserved_demand: u32, congestion: f64, cost: f64) -> Self {
        Self {
            served_demand,
            unserved_demand,
            congestion,
            cost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new(80, 20, 0.5, 12.5);
        assert_eq!(metrics.served_demand, 80);
        assert_eq!(metrics.unserved_demand, 20);
        assert!((metrics.congestion - 0.5).abs() < f64::EPSILON);
        assert!((metrics.cost - 12.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_default() {
        let metrics = Metrics::default();
        assert_eq!(metrics.served_demand, 0);
        assert_eq!(metrics.unserved_demand, 0);
        assert!((metrics.congestion - 0.0).abs() < f64::EPSILON);
        assert!((metrics.cost - 0.0).abs() < f64::EPSILON);
    }
}
