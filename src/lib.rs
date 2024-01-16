pub mod bus;
pub mod consts;
pub mod data;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use bus::BusLocation;
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

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Location {
    pub index: usize,
    id: Uuid,
}

impl Location {
    fn new(index: usize) -> Location {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
enum PassengerStatus {
    OnBus,
    Waiting,
    Arrived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct PassengerOnboardingBusSchedule {
    time_tick: u32,
    // the last destination will not include a bus number because the passenger will be at his destination
    bus_num: Option<usize>,
    stop_location: Location,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
// Passenger should eventually contain the index and the time tick for the bus
// I'm not sure how recalculating the route should work if the passenger doesn't get on the bus
// I wonder if some sort of priority system would be useful sometime
pub struct Passenger {
    id: Uuid,
    destination_location: Location,
    pub current_location: Option<Location>,
    passed_stops: u32,
    bus_schedule: Vec<PassengerOnboardingBusSchedule>,
}
impl Passenger {
    fn new(current_location: Location, destination_location: Location) -> Self {
        Self {
            id: Uuid::new_v4(),
            current_location: Some(current_location),
            destination_location,
            passed_stops: 0,
            bus_schedule: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PassengerBusLocation {
    location: Location,
    location_time_tick: u32,
}

#[derive(Debug)]
pub struct Station {
    pub location: Location,
    pub docked_buses: Vec<bus::SendableBus>,
    passengers: Vec<Passenger>,
}

impl Station {
    pub fn new(location: Location) -> Self {
        Station {
            location,
            docked_buses: Vec::new(),
            passengers: Vec::new(),
        }
    }

    // TODO: Convert to a function returning an Option, figure out
    pub fn add_passenger(
        &mut self,
        mut new_passenger: Passenger,
        time_tick: u32,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    ) -> Result<(), Passenger> {
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule =
            calculate_passenger_schedule_for_bus(new_passenger.clone(), time_tick, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule = new_bus_schedule;
        self.passengers.push(new_passenger);
        Ok(())
    }
}

#[derive(PartialEq, Debug)]
pub enum RejectedPassengersMessages {
    MovingBus,
    StoppedBus { rejected_passengers: Vec<Passenger> },
    CompletedProcessing,
}
#[derive(Debug)]
pub enum StationMessages {
    InitPassengerList(Vec<Passenger>),
    BusArrived(bus::SendableBus),
}

#[derive(Debug, PartialEq, Eq)]
pub enum StationToPassengersMessages {
    ConfirmInitPassengerList(usize),
}
pub enum StationToBusMessages {
    AcknowledgeArrival(),
}

#[derive(PartialEq, Debug)]
pub enum BusMessages {
    InitBus {
        bus_index: usize,
    },
    InitPassengers,
    AdvanceTimeStep {
        // current_time_step: u32,
        bus_index: usize,
    },
    BusFinished {
        bus_index: usize,
    },

    RejectedPassengers(RejectedPassengersMessages),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BusThreadStatus {
    Uninitialized,
    BusFinishedRoute,
    WaitingForTimeStep,
    CompletedTimeStep,
}

fn calculate_passenger_schedule_for_bus(
    passenger: Passenger,
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
    .ok_or(passenger)
}

struct PassengerScheduleWithDistance {
    passenger_schedule: VecDeque<PassengerOnboardingBusSchedule>,
    distance: u32,
}

impl From<VecDeque<PassengerOnboardingBusSchedule>> for PassengerScheduleWithDistance {
    fn from(
        passenger_schedule: VecDeque<PassengerOnboardingBusSchedule>,
    ) -> PassengerScheduleWithDistance {
        let distance = passenger_schedule
            .back()
            .expect("Passenger Schedule must not be empty")
            .time_tick;
        PassengerScheduleWithDistance {
            passenger_schedule,
            distance,
        }
    }
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