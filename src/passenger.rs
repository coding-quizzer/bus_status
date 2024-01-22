use crate::location::Location;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
// Passenger should eventually contain the index and the time tick for the bus
// I'm not sure how recalculating the route should work if the passenger doesn't get on the bus
// I wonder if some sort of priority system would be useful sometime

// #[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
// enum PassengerStatus {
//     OnBus,
//     Waiting,
//     Arrived,
// }
pub struct Passenger {
    pub id: Uuid,
    pub destination_location: Location,
    pub current_location: Option<Location>,
    pub passed_stops: u32,
    pub bus_schedule: Vec<PassengerOnboardingBusSchedule>,
}
impl Passenger {
    pub fn new(current_location: Location, destination_location: Location) -> Self {
        Self {
            id: Uuid::new_v4(),
            current_location: Some(current_location),
            destination_location,
            passed_stops: 0,
            bus_schedule: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PassengerOnboardingBusSchedule {
    pub time_tick: u32,
    // the last destination will not include a bus number because the passenger will be at his destination
    pub bus_num: Option<usize>,
    pub stop_location: Location,
}
