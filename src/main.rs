use bus_system::initialize_location_list;
use bus_system::{generate_bus_route_locations_with_distances, generate_random_passenger_list};
use core::num;
use std::path::Path;

use bus_system::bus::BusLocation;
use bus_system::consts::*;
use bus_system::data::{self, InputDataStructure, SerializedIndexedPassenger};
use bus_system::location::Location;
use bus_system::main_loop::{run_simulation, ConfigStruct};

#[tokio::main]
fn main() {
    simple_logger::SimpleLogger::new()
        .without_timestamps()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();
    let initial_data: InputDataStructure = if READ_JSON {
        println!("Reading data from file");
        data::read_data_from_file(Path::new("bus_route_data.json")).unwrap()
    } else {
        // These need to actually be set
        let mut bus_routes = Vec::new();
        bus_routes.resize(DEFAULT_NUM_OF_BUSES, Vec::new());
        // bus_routes is only used if READ_JSON
        InputDataStructure {
            bus_routes,
            passengers: Vec::new(),
            location_vector: Vec::new(),
        }
    };

    let InputDataStructure {
        bus_routes,
        passengers,
        location_vector,
    } = initial_data;

    let indexed_passengers = passengers
        .into_iter()
        .enumerate()
        .map(|(master_index, passenger)| SerializedIndexedPassenger {
            master_index,
            passenger,
        });

    let num_of_buses = bus_routes.len();
    let num_of_locations = if READ_JSON {
        location_vector.len()
    } else {
        DEFAULT_GLOBAL_LOCATION_COUNT
    };
    let num_of_passengers = if READ_JSON {
        indexed_passengers.len()
    } else {
        GLOBAL_PASSENGER_COUNT
    };

    let location_vector: Vec<Location> = if READ_JSON {
        location_vector
    } else {
        initialize_location_list(DEFAULT_GLOBAL_LOCATION_COUNT)
    };

    let total_passenger_list = if READ_JSON {
        indexed_passengers
            .into_iter()
            .map(|passenger| passenger.into())
            .collect()
    } else {
        generate_random_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap()
    };

    let mut bus_route_vec: Vec<Vec<BusLocation>> = Vec::new();
    bus_route_vec.resize(num_of_buses, Vec::new());
    if READ_JSON {
        bus_route_vec = bus_routes;
    } else {
        for bus_route in bus_route_vec.iter_mut() {
            let next_bus_route = generate_bus_route_locations_with_distances(
                &location_vector,
                NUM_STOPS_PER_BUS,
                MAX_LOCATION_DISTANCE,
            );

            *bus_route = next_bus_route.unwrap();
        }
    }

    // arguments: location vector: Vec<Location>, total_passenger_list: Vec<Passenger>, bus_route_array: [Vec<BusLocation>, NUM_OF_BUSES]
    let config_struct = ConfigStruct {
        num_of_buses,
        num_of_locations,
        bus_capacity: DEFAULT_BUS_CAPACITY as u32,
        num_of_passengers,
    };

    for bus_route in &mut bus_route_vec {
        for location in bus_route {
            location.distance_to_location *= 2; // This is here because run_simulation runs this in terms of steps that it takes, not ticks (2 steps per tick)
        }
    }
    let remaining_passengers = run_simulation(
        location_vector,
        total_passenger_list,
        bus_route_vec,
        config_struct,
    );
    for (index, passenger_location_list) in
        remaining_passengers.location_lists.into_iter().enumerate()
    {
        println!(
            "{:?} passengers arrived at Location {}",
            passenger_location_list.len(),
            index,
        );
    }
    println!("Simulation over");
}
