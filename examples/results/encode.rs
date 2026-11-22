use std::fmt::Write;

use urbanflow::analysis::OperationalAnalysis;
use urbanflow::rail::{RailPosition, RailSnapshot};
use urbanflow::world::{EdgeKind, World};

pub(super) struct Encoder {
    data: String,
    completed_offset: usize,
}

fn check_size(bytes: usize) -> Result<(), String> {
    super::check_size(bytes).map_err(|detail| format!("encoded {detail}"))
}

impl Encoder {
    pub fn new(world: &World) -> Result<Self, String> {
        let mut data = String::from("{\"version\":1,\"nodes\":[");

        for (index, node) in world.nodes.iter().enumerate() {
            if index > 0 {
                data.push(',');
            }

            write!(data, "\"{}\"", node.id.0).unwrap();
            check_size(data.len())?;
        }

        data.push_str("],\"edges\":[");

        for (index, edge) in world.network.edges().iter().enumerate() {
            if index > 0 {
                data.push(',');
            }

            let kind = match edge.kind {
                EdgeKind::Road => "Road",
                EdgeKind::Rail => "Rail",
            };

            write!(
                data,
                r#"{{"id":"{}","from":"{}","to":"{}","kind":"{kind}"}}"#,
                edge.id.0, edge.from.0, edge.to.0
            )
            .unwrap();
            check_size(data.len())?;
        }

        data.push_str("],\"trace\":{\"completed\":");

        let completed_offset = data.len();

        data.push_str("false,\"snapshots\":[");

        check_size(data.len())?;

        Ok(Self {
            data,
            completed_offset,
        })
    }

    pub fn snapshot(&mut self, index: usize, snapshot: &RailSnapshot) -> Result<(), String> {
        let data = &mut self.data;

        if index > 0 {
            data.push(',');
        }

        write!(data, "{{\"tick\":\"{}\",\"position\":", snapshot.tick).unwrap();

        match snapshot.position {
            RailPosition::AtStop { stop_index, node, dwell_ticks_remaining } => write!(
                data,
                r#"{{"kind":"AtStop","stop_index":"{stop_index}","node":"{}","dwell_ticks_remaining":"{dwell_ticks_remaining}"}}"#,
                node.0
            ),
            RailPosition::Traveling { edge_index, edge, from, to, travel_ticks_elapsed, travel_ticks_total } => write!(
                data,
                r#"{{"kind":"Traveling","edge_index":"{edge_index}","edge":"{}","from":"{}","to":"{}","travel_ticks_elapsed":"{travel_ticks_elapsed}","travel_ticks_total":"{travel_ticks_total}"}}"#,
                edge.0, from.0, to.0
            ),
            RailPosition::Complete { stop_index, node } => write!(
                data,
                r#"{{"kind":"Complete","stop_index":"{stop_index}","node":"{}"}}"#,
                node.0
            ),
        }.unwrap();

        write!(
            data,
            ",\"occupancy\":{},\"capacity\":{},\"passengers\":[",
            snapshot.occupancy, snapshot.capacity
        )
        .unwrap();

        for (index, passenger) in snapshot.passengers.iter().enumerate() {
            if index > 0 {
                data.push(',');
            }

            write!(
                data,
                r#"{{"from":"{}","to":"{}","amount":{},"waiting":{},"onboard":{},"arrived":{},"unserved":{}}}"#,
                passenger.demand.origin.0, passenger.demand.destination.0, passenger.demand.amount,
                passenger.waiting, passenger.onboard, passenger.arrived, passenger.unserved
            ).unwrap();
            check_size(data.len())?;
        }

        data.push_str("]}");

        // Every result still needs its trailer. A completed run can shorten "false" to "true".
        let completed = matches!(snapshot.position, RailPosition::Complete { .. });
        check_size(data.len() + r#"]},"analysis":null}"#.len() - usize::from(completed))
    }

    pub fn finish(
        mut self,
        completed: bool,
        analysis: Option<&OperationalAnalysis>,
    ) -> Result<String, String> {
        let data = &mut self.data;

        // A run's completion flag is known only after writing its last snapshot.
        if completed {
            data.replace_range(
                self.completed_offset..self.completed_offset + "false".len(),
                "true",
            );
        }
        data.push_str("]},\"analysis\":");

        if let Some(analysis) = analysis {
            write_analysis(data, analysis)?;
        } else {
            data.push_str("null");
        }

        data.push('}');
        check_size(data.len())?;
        Ok(self.data)
    }
}

fn write_analysis(data: &mut String, analysis: &OperationalAnalysis) -> Result<(), String> {
    let optional = |value: Option<f64>| value.map_or_else(|| "null".into(), |v| v.to_string());
    let outcomes = analysis.passenger_outcomes;

    write!(
        data,
        r#"{{"passenger_outcomes":{{"requested":"{}","arrived":"{}","unserved":"{}","served_share":{}}}"#,
        outcomes.requested, outcomes.arrived, outcomes.unserved, optional(outcomes.served_share),
    ).unwrap();

    let times = analysis.passenger_times;

    write!(
        data,
        r#","passenger_times":{{"waiting_passenger_ticks":"{}","onboard_passenger_ticks":"{}","arrived_waiting_passenger_ticks":"{}","arrived_onboard_passenger_ticks":"{}","mean_waiting_ticks":{},"mean_onboard_ticks":{},"mean_journey_ticks":{}}}"#,
        times.waiting_passenger_ticks, times.onboard_passenger_ticks,
        times.arrived_waiting_passenger_ticks, times.arrived_onboard_passenger_ticks,
        optional(times.mean_waiting_ticks), optional(times.mean_onboard_ticks), optional(times.mean_journey_ticks),
    ).unwrap();

    let occupancy = analysis.vehicle_occupancy;

    write!(
        data,
        r#","vehicle_occupancy":{{"capacity":{},"max_occupancy":{},"occupied_passenger_ticks":"{}","mean_occupancy":{},"max_load_factor":{},"mean_load_factor":{}}}"#,
        occupancy.capacity, occupancy.max_occupancy, occupancy.occupied_passenger_ticks,
        optional(occupancy.mean_occupancy), optional(occupancy.max_load_factor), optional(occupancy.mean_load_factor),
    ).unwrap();

    data.push_str(",\"stops\":[");

    for (index, stop) in analysis.stops.iter().enumerate() {
        if index > 0 {
            data.push(',');
        }

        write!(
            data,
            r#"{{"stop_index":"{}","node":"{}","boarded":"{}","alighted":"{}","remaining_waiting":"{}"}}"#,
            stop.stop_index, stop.node.0, stop.boarded, stop.alighted, stop.remaining_waiting,
        ).unwrap();
        check_size(data.len())?;
    }

    let timing = analysis.route_timing;

    write!(
        data,
        r#"],"route_timing":{{"elapsed_ticks":"{}","traveling_ticks":"{}","dwelling_ticks":"{}","completion_tick":"{}"}}}}"#,
        timing.elapsed_ticks, timing.traveling_ticks, timing.dwelling_ticks, timing.completion_tick,
    ).unwrap();
    Ok(())
}
