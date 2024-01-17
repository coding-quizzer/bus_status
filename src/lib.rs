pub mod bus;
pub mod consts;
pub mod data;
pub mod location;
pub mod passenger;
pub mod station;
pub mod thread;

use passenger::{Passenger, PassengerOnboardingBusSchedule};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use bus::BusLocation;
use location::{Location, PassengerBusLocation};
use std::sync::mpsc::{self, Receiver, Sender};

/// generates a random list from a set of elements such that
/// no two consecutive elements are identical.
fn generate_list_of_random_elements_from_list<T: Copy>(
    // List of elements from which list is generated
    list: &Vec<T>,
    // Number of elements desired in the
    output_length: usize,
) -> Result<Vec<T>, String> {
    use rand::Rng;
    if list.len() <= 1 {
        return Err("List must contain at least 2 elements.".to_string());
    };
    let mut output_list = vec![];
    let mut rng = rand::thread_rng();
    let location_count = list.len();

    let mut old_index = rng.gen_range(0..location_count);
    output_list.push(list[old_index]);

    // Start at one because the first location was pushed before the for loop
    for _ in 1..output_length {
        let mut new_index;
        new_index = rng.gen_range(0..location_count);
        while old_index == new_index {
            new_index = rng.gen_range(0..location_count);
        }
        output_list.push(list[new_index]);
        old_index = new_index;
    }

    Ok(output_list)
}

fn generate_passenger(location_list: &Vec<Location>) -> Result<Passenger, String> {
    let location_vector = generate_list_of_random_elements_from_list(location_list, 2)?;

    let [old_location, new_location] = location_vector[..] else {
        panic!("Returned Vector was invalid")
    };

    Ok(Passenger::new(old_location, new_location))
}

pub fn generate_random_passenger_list(
    count: usize,
    location_list: &Vec<Location>,
) -> Result<Vec<Passenger>, String> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger(location_list)?)
    }

    Ok(passenger_list)
}

pub fn convert_bus_route_list_to_passenger_bus_route_list(
    bus_route_list: Vec<BusLocation>,
) -> Vec<PassengerBusLocation> {
    let mut time_tick = 2;
    let mut passenger_bus_route_list = vec![];
    for bus_location in bus_route_list.iter() {
        let mut index_increment = 0;
        // If this is the not the first location, add two time ticks for waiting at the last station
        if time_tick != 2 {
            // Add one to the index_increment for the time tick used at the previous stop
            index_increment += 2;
        }

        // add time steps for the distance to the destination
        index_increment += bus_location.distance_to_location * 2;

        time_tick += index_increment;

        let passenger_bus_location = PassengerBusLocation {
            location: bus_location.location,
            location_time_tick: time_tick,
        };

        passenger_bus_route_list.push(passenger_bus_location);
    }

    passenger_bus_route_list
}

fn generate_bus_route_locations(
    location_list: &Vec<Location>,
    length: usize,
) -> Result<Vec<Location>, String> {
    let bus_route = generate_list_of_random_elements_from_list(location_list, length)?;

    Ok(bus_route)
}

pub fn generate_bus_route_locations_with_distances(
    location_list: &Vec<Location>,
    length: usize,
    max_distance: u32,
) -> Result<Vec<BusLocation>, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bus_route_list = generate_bus_route_locations(location_list, length);
    let bus_route_list_to_bus_location_types = bus_route_list?
        .iter()
        .map(|location| BusLocation {
            location: *location,
            distance_to_location: rng.gen_range(1..=max_distance),
        })
        .collect::<Vec<_>>();
    Ok(bus_route_list_to_bus_location_types)
}

// fn generate_bus_route_arr

pub fn initialize_location_list(count: usize) -> Vec<Location> {
    let mut location_list = vec![];
    for index in 0..count {
        location_list.push(Location::new(index));
    }
    location_list
}

pub struct ReceiverWithIndex<T> {
    pub receiver: Receiver<T>,
    pub index: usize,
}
pub fn initialize_channel_list<T>(
    channel_count: usize,
) -> (Vec<Sender<T>>, Vec<ReceiverWithIndex<T>>) {
    let mut sender_vector = Vec::new();
    let mut receiver_vector = Vec::new();
    for index in 0..channel_count {
        let (current_sender, current_receiver) = mpsc::channel();
        sender_vector.push(current_sender);
        receiver_vector.push(ReceiverWithIndex {
            receiver: current_receiver,
            index,
        });
    }

    (sender_vector, receiver_vector)
}
