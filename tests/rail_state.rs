use urbanflow::rail::RailRoute;
use urbanflow::world::EdgeId;

#[test]
fn rail_route_preserves_edge_order() {
    let route = RailRoute {
        edges: vec![EdgeId(2), EdgeId(0), EdgeId(1)],
    };

    assert_eq!(route.edges, vec![EdgeId(2), EdgeId(0), EdgeId(1)]);
}
