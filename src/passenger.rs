use crate::location::Location;
use serde::{Deserialize, Serialize};
use std::vec;
use std::{collections::VecDeque, iter::Peekable};
use uuid::Uuid;
// Passenger should eventually contain the index and the time tick for the bus
// I'm not sure how recalculating the route should work if the passenger doesn't get on the bus
// I wonder if some sort of priority system would be useful sometime

// #[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
// enum PassengerStatus {
//     OnBus,
//     Waiting,
//     Arrived,
// }

#[derive(Clone)]
pub struct Passenger {
    pub id: Uuid,
    pub index: usize,
    pub destination_location: Location,
    pub current_location: Option<Location>,
    pub passed_stops: u32,
    pub beginning_time_step: u32,
    pub bus_schedule: Vec<PassengerOnboardingBusSchedule>,
    // Add a peekable iterator for the current location
    pub archived_stop_list: Vec<PassengerOnboardingBusSchedule>,
    pub next_bus_num: Option<usize>,

    pub bus_schedule_iterator: std::iter::Peekable<vec::IntoIter<PassengerOnboardingBusSchedule>>,
}
impl std::fmt::Debug for Passenger {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        f.debug_struct("Passenger")
            .field("id", &self.id)
            .field("destination_location", &self.destination_location)
            .field("current_location", &self.current_location)
            .field("passed_stops", &self.passed_stops)
            .field("bus_schedule", &self.bus_schedule)
            .field("archived_stop_list", &self.archived_stop_list)
            .field("next_bus_num", &self.next_bus_num)
            .finish()
    }
}

impl std::fmt::Display for Passenger {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        write!(f, "Passenger {}", self.index)
    }
}

impl PartialEq for Passenger {
    fn eq(&self, other: &Passenger) -> bool {
        self.id == other.id
            && self.destination_location == other.destination_location
            && self.current_location == other.current_location
            && self.passed_stops == other.passed_stops
            && self.bus_schedule == other.bus_schedule
            && self.archived_stop_list == other.archived_stop_list
    }
}

impl Eq for Passenger {}

impl Passenger {
    pub fn new(
        current_location: Location,
        destination_location: Location,
        beginning_time_step: u32,
        index: usize,
    ) -> Self {
        let bus_schedule: Vec<PassengerOnboardingBusSchedule> = Vec::new();
        // Since bus_schedule is cloned for bus_schedul_iter, the values do not actually correlate with each other
        let bus_schedule_iter: Peekable<std::vec::IntoIter<PassengerOnboardingBusSchedule>> =
            bus_schedule.clone().into_iter().peekable();
        Self {
            id: Uuid::new_v4(),
            index,
            current_location: Some(current_location),
            destination_location,
            passed_stops: 0,
            beginning_time_step,
            bus_schedule,
            next_bus_num: None,
            archived_stop_list: Vec::new(),
            bus_schedule_iterator: bus_schedule_iter,
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

// #[derive(Clone, Debug)]
// pub struct PassengerInfo {
//     pub passenger: Passenger,
//     pub current_location_index: usize,
// }

/* impl PassengerInfo {
    pub fn new(passenger: Passenger, current_location_index: usize) -> PassengerInfo {
        PassengerInfo {
            passenger,
            current_location_index,
        }
    }
} */

/* impl From<Passenger> for PassengerInfo {
    fn from(passenger: Passenger) -> PassengerInfo {
        PassengerInfo {
            passenger,
            current_location_index: 0,
        }
    }
}
 */
