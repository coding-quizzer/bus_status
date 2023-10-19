use std::{
    ops::ControlFlow,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};
use uuid::Uuid;

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

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Location {
    id: Uuid,
}

impl Location {
    fn new() -> Location {
        Location { id: Uuid::new_v4() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PassengerStatus {
    OnBus,
    Waiting,
    Arrived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Passenger {
    id: Uuid,
    destination_location: Location,
    status: PassengerStatus,
    current_location: Option<Location>,
    passed_stops: u32,
}
impl Passenger {
    fn new(current_location: Location, destination_location: Location) -> Self {
        Self {
            id: Uuid::new_v4(),
            current_location: Some(current_location),
            destination_location,
            status: PassengerStatus::Waiting,
            passed_stops: 0,
        }
    }

    fn convert_to_onboarded_passenger(mut self) -> Self {
        if self.status == PassengerStatus::Waiting {
            self.status = PassengerStatus::OnBus;
            self.current_location = None;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PassengerOnBus {
    id: Uuid,
    end_location: Location,
    passed_stops: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PassengerWaiting {
    id: Uuid,
    current_location: Location,
    end_location: Location,
}

impl PassengerWaiting {
    fn new(current_location: Location, end_location: Location) -> PassengerWaiting {
        PassengerWaiting {
            id: Uuid::new_v4(),
            current_location,
            end_location,
        }
    }

    fn convert_to_onboarded_passenger(&self) -> PassengerOnBus {
        PassengerOnBus {
            id: self.id,
            end_location: self.end_location,
            passed_stops: 0,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum MovementState {
    // Moving contains the distance to the next location
    Moving(u32),
    Stopped,
    Finished,
}

impl Default for MovementState {
    fn default() -> Self {
        Self::Moving(0)
    }
}

#[derive(Debug, Clone)]
struct BusStatus {
    movement: MovementState,
}

// clone iterator from: https://stackoverflow.com/questions/49594732/how-to-return-a-boxed-clonable-iterator-in-rust/49599226#49599226
trait CloneIterator: Iterator {
    fn clone_box(&self) -> Box<dyn CloneIterator<Item = Self::Item>>;
}

impl<T> CloneIterator for T
where
    T: 'static + Iterator + Clone,
{
    fn clone_box(&self) -> Box<dyn CloneIterator<Item = Self::Item>> {
        Box::new(self.clone())
    }
}

// Evantually, Passengers will need to somehow access the bus timetable to determine what bus will reach
// their destination. So far, the bus has decided what passengers can get on, but at this point
// the passengers should probably choose.

// The current bus route data the buses impliment may not be the same format the passengers would want
// It's hard to compare the positions of the buses at each time tick.s
struct Bus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_iter: Box<dyn CloneIterator<Item = BusLocation>>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    total_passenger_count: u32,
    time_tick_num: u32,
    bus_num: usize,
}

// manually impliment Debug, so that the iterator field can be skipped, eliminating the complicaiton of requiring
// the iterator to impliment Debug
impl std::fmt::Debug for Bus {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        f.debug_struct("Bus")
            .field("status", &self.status)
            .field("passengers", &self.passengers)
            .field("current_location", &self.current_location)
            .field("location_vec", &&self.bus_route_vec)
            .finish()
    }
}

// let passengers take the most efficient route

#[derive(Debug, Clone)]
struct BusLocation {
    location: Location,
    // distance_to_next is None for the last location
    distance_to_location: u32,
}

impl Bus {
    fn new(bus_route: Vec<BusLocation>, capacity: usize, bus_num: usize) -> Bus {
        let bus_route_vec = bus_route.clone();
        let mut iterator = bus_route.into_iter();
        let first_bus_location = iterator
            .next()
            .expect("Bus route must contain at least one location");
        let BusLocation {
            location: first_bus_location,
            distance_to_location: distance_to_first_location,
        } = first_bus_location;
        Bus {
            status: BusStatus {
                movement: MovementState::Moving(distance_to_first_location),
            },
            passengers: vec![],
            current_location: Some(first_bus_location),
            bus_route_iter: Box::new(iterator),
            bus_route_vec,
            capacity,
            total_passenger_count: 0,
            time_tick_num: 0,
            bus_num,
        }
    }

    fn stop_at_destination_stop(&mut self) -> Option<()> {
        self.status.movement = MovementState::Stopped;
        Some(())
    }

    fn leave_for_next_location(&mut self) -> Option<()> {
        // Return None if the previous location was the end of the list
        let next_location = self.bus_route_iter.next()?;
        self.current_location = Some(next_location.location);

        self.status.movement = MovementState::Moving(next_location.distance_to_location);
        Some(())
    }

    fn add_passenger(&mut self, passenger: &Passenger) {
        self.passengers.push(*passenger);
    }

    fn update(
        &mut self,
        waiting_passengers: &mut Vec<Passenger>,
        passenger_stops_waited_list: &mut Vec<u32>,
        sender: &Sender<BusMessages>, // bus_num: u32,
        current_time_tick_number: &u32,
    ) -> ControlFlow<()> {
        if self.time_tick_num < *current_time_tick_number {
            sender
                .send(BusMessages::AdvanceTimeStep {
                    //current_time_step: self.time_tick_num,
                    bus_number: self.bus_num,
                })
                .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
            println!("Bus Number {} Sent", self.bus_num);
            if let MovementState::Moving(distance) = self.status.movement {
                if distance > 0 {
                    println!("Bus {} distance to next stop: {}", self.bus_num, distance);
                    self.status.movement = MovementState::Moving(distance - 1);
                    // return Some(());
                } else {
                    self.stop_at_destination_stop();
                }
                self.time_tick_num += 1;
            } else {
                let more_locations_left = self.leave_for_next_location();

                self.drop_off_passengers(passenger_stops_waited_list);
                self.take_passengers(waiting_passengers);

                self.time_tick_num += 1;

                if more_locations_left.is_some() {
                    return ControlFlow::Continue(());
                };

                assert_eq!(self.passengers.len(), 0);
                self.status.movement = MovementState::Finished;
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }

    fn take_passengers(&mut self, waiting_passengers: &mut Vec<Passenger>) {
        let mut new_passengers = vec![];
        for passenger in &mut *waiting_passengers {
            if self.passengers.len() >= self.capacity {
                break;
            }

            let mut cloned_locations = self.bus_route_iter.clone_box();

            // might become a seperate function call
            let bus_will_stop_at_passengers_location = cloned_locations
                .any(|location_of_bus| location_of_bus.location == passenger.destination_location);

            if bus_will_stop_at_passengers_location {
                self.current_location.map_or((), |loc| {
                    if loc == passenger.current_location.unwrap() {
                        let onboard_passenger = passenger.convert_to_onboarded_passenger();
                        self.add_passenger(&onboard_passenger);
                        new_passengers.push(passenger.clone());
                    }
                })
            }
        }

        for passenger in new_passengers {
            // remove_from_list(passenger_list, passenger);
            waiting_passengers.retain(|pass| pass.clone() != passenger);
        }
    }
    fn drop_off_passengers(&mut self, passenger_passed_stops: &mut Vec<u32>) -> Option<()> {
        println!("Drop off Passengers");
        let current_location = self.current_location?;
        let bus_passengers = &mut *self.passengers;
        let mut new_bus_passengers = vec![];
        for pass in bus_passengers {
            if pass.destination_location == current_location {
                println!("Passenger left Bus {}", self.bus_num);
                passenger_passed_stops.push(pass.passed_stops);
                self.total_passenger_count += 1;
            } else {
                pass.passed_stops += 1;
                new_bus_passengers.push(*pass);
            }
        }
        self.passengers = new_bus_passengers;
        Some(())
    }

    fn get_bus_route(&self) -> Vec<BusLocation> {
        self.bus_route_vec.clone()
    }
}

#[derive(PartialEq, Debug)]
enum BusMessages {
    AdvanceTimeStep {
        // current_time_step: u32,
        bus_number: usize,
    },
    BusFinished {
        bus_number: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BusThreadStatus {
    BusFinishedRoute,
    WaitingForTimeStep,
    CompletedTimeStep,
}

fn generate_passenger(location_list: &Vec<Location>) -> Result<Passenger, String> {
    let location_vector = generate_list_of_random_elements_from_list(location_list, 2)?;

    let [old_location, new_location] = location_vector[..] else {
        panic!("Returned Vector was invalid")
    };

    Ok(Passenger::new(old_location, new_location))
}

fn generate_passenger_list(
    count: u32,
    location_list: &Vec<Location>,
) -> Result<Vec<Passenger>, String> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger(location_list)?)
    }

    Ok(passenger_list)
}

fn generate_bus_route_locations(
    location_list: &Vec<Location>,
    length: usize,
) -> Result<Vec<Location>, String> {
    let bus_route = generate_list_of_random_elements_from_list(location_list, length)?;

    Ok(bus_route)
}

fn generate_bus_route_locations_with_distances(
    location_list: &Vec<Location>,
    length: usize,
) -> Result<Vec<BusLocation>, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bus_route_list = generate_bus_route_locations(location_list, length);
    let bus_route_list_to_bus_location_types = bus_route_list?
        .iter()
        .map(|location| BusLocation {
            location: *location,
            distance_to_location: rng.gen_range(1..=5),
        })
        .collect::<Vec<_>>();
    Ok(bus_route_list_to_bus_location_types)
}

fn initialize_location_list(count: u32) -> Vec<Location> {
    let mut location_list = vec![];
    for _ in 0..count {
        location_list.push(Location::new());
    }
    location_list
}

const GLOBAL_PASSENGER_COUNT: u32 = 500;
const GLOBAL_LOCATION_COUNT: u32 = 10;
const BUS_CAPACITY: usize = 10;
const NUM_OF_BUSES: usize = 4;
const NUM_STOPS_PER_BUS: usize = 25;

fn main() {
    let location_vector = initialize_location_list(GLOBAL_LOCATION_COUNT);

    let passenger_list_pointer = Arc::new(Mutex::new(
        generate_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap(),
    ));

    let location_vector_arc = Arc::new(location_vector);

    let bus_route_vec_arc = Arc::new(Mutex::new(vec![]));

    let passenger_extra_stops_waited_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    let current_time_tick = Arc::new(Mutex::new(1));

    let mut handle_list = vec![];

    let (tx_from_threads, rx_from_threads) = mpsc::channel();

    let current_time_tick_clone = current_time_tick.clone();
    let route_sync_handle = thread::spawn(move || {
        let mut bus_status_array = [BusThreadStatus::WaitingForTimeStep; NUM_OF_BUSES];

        loop {
            let received_bus_stop_message = rx_from_threads.recv().unwrap();

            match received_bus_stop_message {
                BusMessages::AdvanceTimeStep {
                    // current_time_step,
                    bus_number,
                    ..
                } => {
                    println!("Stop recieved from Bus {}", bus_number);
                    bus_status_array[bus_number - 1] = BusThreadStatus::CompletedTimeStep;
                }

                BusMessages::BusFinished { bus_number } => {
                    bus_status_array[bus_number - 1] = BusThreadStatus::BusFinishedRoute;
                    println!("Bus {} Finished Route", bus_number);
                    let finished_buses = bus_status_array
                        .iter()
                        .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                        .count();
                    let active_buses = NUM_OF_BUSES - finished_buses;
                    println!("There are now {active_buses} active buses.");
                }
            }

            let finished_buses = bus_status_array
                .iter()
                .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                .count();

            if finished_buses >= NUM_OF_BUSES {
                println!("Program Complete");
                break;
            }

            // Increment the time step after all buses have run
            if bus_status_array
                .iter()
                .filter(|status| **status == BusThreadStatus::WaitingForTimeStep)
                .count()
                == 0
            {
                for status in bus_status_array.iter_mut() {
                    if status == &BusThreadStatus::CompletedTimeStep {
                        *status = BusThreadStatus::WaitingForTimeStep;
                    }
                }
                println!("---------- All buses are finished at their stops -----------");
                *current_time_tick_clone.lock().unwrap() += 1;
                // buses_finished_at_stops = 0;
            }
        }
    });

    handle_list.push(route_sync_handle);

    for bus_num in 1..=NUM_OF_BUSES {
        let sender = tx_from_threads.clone();
        let bus_route = generate_bus_route_locations_with_distances(
            location_vector_arc.clone().as_ref(),
            NUM_STOPS_PER_BUS,
        )
        .unwrap();
        let passenger_list_pointer_clone = passenger_list_pointer.clone();
        let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let bus_stop_number_clone = current_time_tick.clone();
        let bus_route_vec_clone = bus_route_vec_arc.clone();
        let handle = thread::spawn(move || {
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY, bus_num);
            bus_route_vec_clone
                .lock()
                .unwrap()
                .push(simulated_bus.get_bus_route());

            loop {
                let update_option = simulated_bus.update(
                    &mut passenger_list_pointer_clone.lock().unwrap(),
                    &mut passenger_stops_passed_pointer_clone.lock().unwrap(),
                    &sender,
                    &bus_stop_number_clone.lock().unwrap(),
                );

                match update_option {
                    ControlFlow::Break(()) => break,
                    ControlFlow::Continue(()) => {}
                }
            }
            sender
                .send(BusMessages::BusFinished {
                    bus_number: simulated_bus.bus_num,
                })
                .unwrap();
        });
        handle_list.push(handle);
    }
    for handle in handle_list {
        handle.join().unwrap();
    }

    let passenger_extra_stops_waited = passenger_extra_stops_waited_pointer.lock().unwrap();
    let total_extra_stops_waited: u32 = passenger_extra_stops_waited.iter().sum();

    let total_passengers = passenger_extra_stops_waited.len();

    let passengers_remaining = passenger_list_pointer.lock().unwrap().len();

    println!("{passengers_remaining} passengers did not get onto any bus");

    println!("total extra stops waited: {}", total_extra_stops_waited);

    println!(
        "Average wait time between stops {}",
        total_extra_stops_waited as f64 / total_passengers as f64
    );
}
