use crate::consts::{DEFAULT_GLOBAL_LOCATION_COUNT, DEFAULT_NUM_OF_BUSES, GLOBAL_PASSENGER_COUNT};
use crate::Passenger;
use crate::{BusLocation, Location};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
pub struct InputDataStructure {
    pub bus_routes: Vec<Vec<BusLocation>>,
    pub passengers: Vec<SerializedPassenger>,
    pub location_vector: Vec<Location>,
}

/* struct ConvertedInputDataStructure {
  bus_routes: Vec<Vec<BusLocation>>,

} */

fn locations_are_all_valid(
    mut locations_to_test: impl Iterator<Item = Location>,
    location_master_list: &[Location],
) -> bool {
    // test every location
    !locations_to_test.any(|tested_location| {
        !location_master_list
            .iter()
            .any(|master_list| master_list == &tested_location)
    })
}

pub fn read_data_from_file(path: &Path) -> Result<InputDataStructure, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Data`.
    let data: InputDataStructure = serde_json::from_reader(reader)?;

    let InputDataStructure {
        bus_routes,
        passengers,
        location_vector,
    } = data.clone();

    assert_eq!(location_vector.len(), DEFAULT_GLOBAL_LOCATION_COUNT);
    assert_eq!(bus_routes.len(), DEFAULT_NUM_OF_BUSES);
    assert_eq!(passengers.len(), GLOBAL_PASSENGER_COUNT);

    let bus_locations = bus_routes
        .iter()
        .flatten()
        .map(|bus_location| bus_location.location);
    // .collect::<Vec<_>>();
    assert!(locations_are_all_valid(bus_locations, &location_vector));

    let passenger_iter = passengers.iter();

    let passenger_locations = passenger_iter.flat_map(|passenger| {
        [
            (passenger.current_location).unwrap(),
            passenger.destination_location,
        ]
    });

    assert!(locations_are_all_valid(
        passenger_locations,
        &location_vector
    ));

    // Return the `Data`.
    Ok(data)
}

pub fn write_data_to_file(data: InputDataStructure, path: &Path) -> Result<(), Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::create(path)?;
    let writer = BufWriter::new(file);

    // Read the JSON contents of the file as an instance of `Data`.
    serde_json::to_writer_pretty(writer, &data)?;

    // Return the `Data`.
    Ok(())
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerializedPassenger {
    id: Uuid,
    destination_location: Location,
    current_location: Option<Location>,
    beginning_time_step: u32,
}

pub struct SerializedIndexedPassenger {
    pub passenger: SerializedPassenger,
    pub master_index: usize,
}

impl From<SerializedIndexedPassenger> for Passenger {
    fn from(serialized_passenger_input: SerializedIndexedPassenger) -> Passenger {
        let SerializedPassenger {
            id,
            destination_location,
            current_location,
            beginning_time_step,
        } = serialized_passenger_input.passenger;

        let bus_schedule = Vec::new();
        let bus_schedule_iterator = bus_schedule.clone().into_iter().peekable();

        Passenger {
            id,
            id_for_display: serialized_passenger_input.master_index,
            destination_location,
            current_location,
            passed_stops: 0,
            bus_schedule,
            archived_stop_list: Vec::new(),
            next_bus_num: None,
            beginning_time_step,
            bus_schedule_iterator,
        }
    }
}

impl From<Passenger> for SerializedPassenger {
    fn from(passenger: Passenger) -> SerializedPassenger {
        let Passenger {
            id,
            id_for_display: _,
            destination_location,
            current_location,
            passed_stops: _passed_stops,
            bus_schedule: _bus_schedule,
            archived_stop_list: _,
            bus_schedule_iterator: _,
            // TODO: transfer value from serialized passenger
            beginning_time_step,
            next_bus_num: _,
        } = passenger;

        SerializedPassenger {
            id,
            destination_location,
            current_location,
            beginning_time_step,
        }
    }
}
