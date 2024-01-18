use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Location {
    pub index: usize,
    pub id: Uuid,
}

impl Location {
    pub fn new(index: usize) -> Location {
        Location {
            id: Uuid::new_v4(),
            index,
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Location {}", self.index)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BusLocation {
    pub location: Location,
    // distance_to_next is None for the last location
    pub distance_to_location: u32,
}

#[derive(Debug, Clone)]
pub struct PassengerBusLocation {
    pub location: Location,
    pub location_time_tick: u32,
}
