pub mod bus;
pub mod consts;
pub mod data;
pub mod location;
pub mod passenger;
pub mod station;
pub mod thread;

use passenger::{Passenger, PassengerOnboardingBusSchedule};
use station::PassengerScheduleWithDistance;
use std::collections::VecDeque;

use bus::BusLocation;
use location::{Location, PassengerBusLocation};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum TimeTickStage {
    #[default]
    PassengerInit,
    BusUnloadingPassengers,
    BusLoadingPassengers,
}

#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct TimeTick {
    pub number: u32,
    pub stage: TimeTickStage,
}

impl TimeTick {
    pub fn increment_time_tick(&mut self) {
        let caller_location = std::panic::Location::caller().line();
        match self.stage {
            TimeTickStage::PassengerInit => {
                self.number = 1;
                self.stage = TimeTickStage::BusUnloadingPassengers;
            }
            TimeTickStage::BusUnloadingPassengers => {
                self.stage = TimeTickStage::BusLoadingPassengers
            }

            TimeTickStage::BusLoadingPassengers { .. } => {
                println!("Time tick number incremented from {}", self.number);
                self.number += 1;
                self.stage = TimeTickStage::BusUnloadingPassengers
            }
        }
    }

    pub fn increment_from_initialized(&mut self) -> Option<()> {
        if self.stage == TimeTickStage::PassengerInit {
            self.number = 1;
            self.stage = TimeTickStage::BusUnloadingPassengers;
        } else {
            panic!("Init");
        }

        Some(())
    }
}

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
    let mut time_tick = 0;
    let mut passenger_bus_route_list = vec![];
    for bus_location in bus_route_list.iter() {
        let mut index_increment = 0;
        // If this is the not the first location, add two time ticks for waiting at the last station
        if time_tick != 1 {
            // Add one to the index_increment for the time tick used at the previous stop
            index_increment += 1;
        }

        // add time steps for the distance to the destination
        index_increment += bus_location.distance_to_location;

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

pub fn calculate_passenger_schedule_for_bus_check_available_buses<'a>(
    new_passenger: &'a Passenger,
    current_time_tick: u32,
    bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    buses_unavailable: Vec<usize>,
) -> Result<Vec<PassengerOnboardingBusSchedule>, Passenger> {
    // TODO: Add tests to prove that this method of removing those indeces actually work
    let mut new_bus_route_list = bus_route_list.clone();

    // mutable iterator containing the routes that will need to remove the
    let special_routes_iter =
        new_bus_route_list
            .iter_mut()
            .enumerate()
            .filter(|(bus_index, bus_route)| {
                buses_unavailable.iter().any(|bus: &usize| bus == bus_index)
            });

    for (_, route) in special_routes_iter {
        let mut index_to_delete: Option<usize> = None;
        for (index_in_route, location) in route.iter().enumerate() {
            if current_time_tick == location.location_time_tick {
                index_to_delete = Some(index_in_route);
                break;
            }
        }

        if let Some(index) = index_to_delete {
            route.remove(index);
        }
    }

    println!("New Bus Route List: {:#?}", &new_bus_route_list);

    // Take bus routes with the same time tick in unavailable buses out

    calculate_passenger_schedule_for_bus(&new_passenger, current_time_tick, &new_bus_route_list)
}

pub fn calculate_passenger_schedule_for_bus<'a>(
    passenger: &'a Passenger,
    current_time_tick: u32,
    bus_route_list: &Vec<Vec<PassengerBusLocation>>,
) -> Result<Vec<PassengerOnboardingBusSchedule>, Passenger> {
    calculate_passenger_schedule_for_bus_with_recursion(
        passenger.current_location.unwrap(),
        passenger.destination_location,
        None,
        current_time_tick,
        bus_route_list,
        Vec::new(),
        None,
    )
    .map(|schedule| schedule.into())
    .ok_or(passenger.clone())
}

fn calculate_passenger_schedule_for_bus_with_recursion(
    initial_location: Location,
    destination_location: Location,
    destination_time_tick: Option<u32>,
    current_time_tick: u32,
    bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    visited_locations: Vec<Location>,
    next_bus_index: Option<usize>,
) -> Option<VecDeque<PassengerOnboardingBusSchedule>> {
    let mut valid_schedules: Vec<PassengerScheduleWithDistance> = Vec::new();
    let mut destination_list = Vec::new();
    // let mut bus_schedule: VecDeque<PassengerOnboardingBusSchedule> = VecDeque::new();

    // Find the index of the bus route containing the destination location and the bus location containting the location and the time tick
    for (bus_index, bus_route) in bus_route_list.iter().enumerate() {
        for bus_location in bus_route {
            if bus_location.location == destination_location
                && current_time_tick <= bus_location.location_time_tick
                // The new destination is not identical to the old destination
                && !((next_bus_index.is_some() && (next_bus_index.unwrap() == bus_index))
                    && (destination_time_tick.is_some()
                        && destination_time_tick.unwrap() == bus_location.location_time_tick))
                && (destination_time_tick.is_none()
                    || destination_time_tick.unwrap() >= bus_location.location_time_tick)
            {
                destination_list.push((bus_index, bus_location));

                /* bus schedule should not contain both potential destinations */
                // bus_schedule.push_front(PassengerOnboardingBusSchedule {
                //     time_tick: bus_location.location_time_tick,
                //     bus_num: Some(bus_index),
                // });
            }
        }
    }

    for destination in destination_list.iter() {
        let mut bus_schedule = VecDeque::new();
        let mut visited_locations = visited_locations.clone();

        // let (destination_bus_index, destination_passenger_bus_location) = destination;
        // let trial_destination_time_tick = destination_passenger_bus_location.location_time_tick;
        // bus_schedule.push_front(PassengerOnboardingBusSchedule {
        //     stop_location: destination_passenger_bus_location.location,
        //     time_tick: trial_destination_time_tick,
        //     bus_num: next_bus_index,
        // });
        let (destination_bus_index, destination_passenger_bus_location) = destination;
        let trial_destination_time_tick = destination_passenger_bus_location.location_time_tick;

        bus_schedule.push_front(PassengerOnboardingBusSchedule {
            stop_location: destination_passenger_bus_location.location,
            time_tick: trial_destination_time_tick,
            bus_num: next_bus_index,
        });
        visited_locations.push(destination_passenger_bus_location.location);
        // Currently, current_bus_index and destination_bus_index mean the same thing
        // let current_bus_index = destination.0;
        let bus_route_for_destination_bus = &bus_route_list[*destination_bus_index];

        for bus_location in bus_route_for_destination_bus {
            // exclude the destination location just added
            #[allow(unused_parens)]
            if bus_location.location_time_tick <= trial_destination_time_tick
                && (destination_time_tick.is_none()
                    || (destination_time_tick.unwrap() >= current_time_tick))
                && (bus_location.location == initial_location)
            {
                let mut final_bus_schedule = bus_schedule.clone();

                final_bus_schedule.push_front(PassengerOnboardingBusSchedule {
                    time_tick: bus_location.location_time_tick,
                    stop_location: bus_location.location,
                    bus_num: Some(*destination_bus_index),
                });

                valid_schedules.push(final_bus_schedule.into());

                continue;
            } else if ((bus_location.location_time_tick < destination_passenger_bus_location.location_time_tick)
                    &&
                    // exclude locations in visited locations
                    !visited_locations
                        .clone()
                        .iter()
                        .any(|location| -> bool { location == &bus_location.location }))
            {
                let extension_bus_route = calculate_passenger_schedule_for_bus_with_recursion(
                    initial_location,
                    bus_location.location,
                    Some(bus_location.location_time_tick),
                    current_time_tick,
                    bus_route_list,
                    visited_locations.clone(),
                    Some(*destination_bus_index),
                );

                match extension_bus_route {
                    Some(mut bus_route) => {
                        // Tack the destination location to the end of the returned list
                        bus_route.push_back(PassengerOnboardingBusSchedule {
                            time_tick: destination_passenger_bus_location.location_time_tick,
                            stop_location: destination_passenger_bus_location.location,
                            bus_num: next_bus_index,
                        });
                        valid_schedules.push(bus_route.into());
                        continue;
                    }
                    None => continue,
                }
            }
        }
    }
    if valid_schedules.is_empty() {
        None
    } else {
        let optimal_schedule = valid_schedules
            .into_iter()
            .min_by(|trial_schedule, accum| (trial_schedule.distance).cmp(&accum.distance))
            .expect("Schedule must contain elements at this point");

        Some(optimal_schedule.passenger_schedule)
    }
}
