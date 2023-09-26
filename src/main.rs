use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};
use uuid::Uuid;

type Passenger = PassengerWaiting;

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

#[derive(Default, Clone, PartialEq, Eq, Debug)]
enum MovementState {
    #[default]
    Moving,
    Stopped,
    Finished,
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
struct Bus {
    status: BusStatus,
    passengers: Vec<PassengerOnBus>,
    current_location: Option<Location>,
    bus_route_iter: Box<dyn CloneIterator<Item = Location>>,
    bus_route_vec: Vec<Location>,
    capacity: usize,
    total_passenger_count: u32,
    bus_stop_num: u32,
    bus_num: u32,
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

// call update twice. How long does it take to get to a stop,
// how long will a stop take

// measure how time
// count system cycles
// coordinate the thread cycles
// process signalling or spin use global variable

// make sure bus yields between cycles
// time between stops
// could correlate number of updates to distance

// let passengers take the most efficient route
//

impl Bus {
    fn new(bus_route: Vec<Location>, capacity: usize, bus_num: u32) -> Bus {
        let bus_route_vec = bus_route.clone();
        let iterator = bus_route.into_iter();
        Bus {
            status: BusStatus {
                movement: MovementState::Moving,
            },
            passengers: vec![],
            current_location: None,
            bus_route_iter: Box::new(iterator),
            bus_route_vec,
            capacity,
            total_passenger_count: 0,
            bus_stop_num: 0,
            bus_num,
        }
    }

    fn stop_at_next_location(&mut self) -> Option<()> {
        self.current_location = self.bus_route_iter.next();
        // Return None if the bus does not have a current location
        self.current_location?;

        self.status.movement = MovementState::Stopped;
        Some(())
    }

    fn add_passenger(&mut self, passenger: &PassengerOnBus) {
        self.passengers.push(*passenger);
    }

    fn update(
        &mut self,
        waiting_passengers: &mut Vec<PassengerWaiting>,
        passenger_stops_waited_list: &mut Vec<u32>,
        sender: &Sender<u32>, // bus_num: u32,
        current_bus_stop_number: &u32,
        bus_count_pointer: &mut u32,
    ) -> Option<()> {
        if self.status.movement != MovementState::Moving {
            self.drop_off_passengers(passenger_stops_waited_list);
            self.take_passengers(waiting_passengers);
            // self.bus_stop_num += 1;
            sender
                .send(self.bus_stop_num)
                .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
            println!("Bus Number {} Sent", self.bus_num);
            //.unwrap()
            self.status.movement = MovementState::Moving;
        } else if self.bus_stop_num < *current_bus_stop_number {
            self.bus_stop_num += 1;
            let stop_output_option = self.stop_at_next_location();
            if stop_output_option.is_some() {
                return Some(());
            };
            assert_eq!(self.passengers.len(), 0);
            self.status.movement = MovementState::Finished;
            *bus_count_pointer -= 1;
            println!("A bus was removed from the bus count");
            println!("Current Bus Count: {}", bus_count_pointer);
            return None;
        }
        Some(())
    }

    fn take_passengers(&mut self, waiting_passengers: &mut Vec<PassengerWaiting>) {
        let mut new_passengers = vec![];
        for passenger in &mut *waiting_passengers {
            if self.passengers.len() == self.capacity {
                break;
            }

            let mut cloned_locations = self.bus_route_iter.clone_box();

            let bus_will_stop_at_passengers_location =
                cloned_locations.any(|location| location == passenger.end_location);

            if bus_will_stop_at_passengers_location {
                self.current_location.map_or((), |loc| {
                    if loc == passenger.current_location {
                        let onboard_passenger = passenger.convert_to_onboarded_passenger();
                        self.add_passenger(&onboard_passenger);
                        new_passengers.push(passenger.clone());
                    }
                })
            } /* else {
                  println!("Passenger's destination: {:?} will not be reached by bus. Passenger did not get on bus", passenger.end_location);
              } */
        }

        for passenger in new_passengers {
            // remove_from_list(passenger_list, passenger);
            waiting_passengers.retain(|pass| pass.clone() != passenger);
        }
    }
    fn drop_off_passengers(&mut self, passenger_passed_stops: &mut Vec<u32>) -> Option<()> {
        let current_location = self.current_location?;
        let bus_passengers = &mut *self.passengers;
        let mut new_bus_passengers = vec![];
        for pass in bus_passengers {
            if pass.end_location == current_location {
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
}

fn generate_passenger(location_list: &Vec<Location>) -> Result<PassengerWaiting, String> {
    let location_vector = generate_list_of_random_elements_from_list(location_list, 2)?;

    let [old_location, new_location] = location_vector[..] else {
        panic!("Returned Vector was invalid")
    };

    Ok(Passenger::new(old_location, new_location))
}

fn generate_passenger_list(
    count: u32,
    location_list: &Vec<Location>,
) -> Result<Vec<PassengerWaiting>, String> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger(location_list)?)
    }

    Ok(passenger_list)
}

fn generate_bus_route(
    location_list: &Vec<Location>,
    length: usize,
) -> Result<Vec<Location>, String> {
    let bus_route = generate_list_of_random_elements_from_list(location_list, length)?;

    Ok(bus_route)
}

fn generate_location_list(count: u32) -> Vec<Location> {
    let mut location_list = vec![];
    for _ in 0..count {
        location_list.push(Location::new());
    }
    location_list
}

const GLOBAL_PASSENGER_COUNT: u32 = 500;
const GLOBAL_LOCATION_COUNT: u32 = 10;
const BUS_CAPACITY: usize = 10;
const NUM_OF_BUSES: u32 = 4;
const NUM_STOPS_PER_BUS: usize = 25;
fn main() {
    let location_vector = generate_location_list(GLOBAL_LOCATION_COUNT);

    let passenger_list_pointer = Arc::new(Mutex::new(
        generate_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap(),
    ));

    let location_vector_arc = Arc::new(location_vector);

    let passenger_extra_stops_waited_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    let potential_running_bus_count = Arc::new(Mutex::new(NUM_OF_BUSES));

    let current_bus_stop = Arc::new(Mutex::new(1));

    let mut handle_list = vec![];

    let (tx_from_threads, rx_from_threads) = mpsc::channel();
    // let (tx to_threads, rx_to_threads) = mpsc::channel();

    let potential_running_bus_count_route_sync_clone = potential_running_bus_count.clone();
    let route_sync_handle = thread::spawn(move || {
        // Somehow deal with the possibility of adding another bus
        let buses_finished_at_stops = 0;
        loop {
            let _received_bus_stop_messages = rx_from_threads.recv().unwrap();
            println!("Bus stop recieved");

            if *potential_running_bus_count_route_sync_clone.lock().unwrap() == 0 {
                println!("Bus count empty");
                break;
            }
        }
    });

    handle_list.push(route_sync_handle);

    // Sends a message to all the buses to signal that they have passed the first bus stop
    // how does the main thread know how many buses to wait for before
    for bus_num in 1..=NUM_OF_BUSES {
        let sender = tx_from_threads.clone();
        let bus_route =
            generate_bus_route(location_vector_arc.clone().as_ref(), NUM_STOPS_PER_BUS).unwrap();
        let passenger_list_pointer_clone = passenger_list_pointer.clone();
        let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let potential_running_bus_count_clone = potential_running_bus_count.clone();
        let bus_stop_number_clone = current_bus_stop.clone();
        let handle = thread::spawn(move || {
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY, bus_num);

            loop {
                // let mut passenger_list = passenger_list_pointer_clone.lock().unwrap();
                // let mut passenger_extra_stops_passed_list =
                //     passenger_wait_pointer_clone.lock().unwrap();

                let update_option = simulated_bus.update(
                    &mut passenger_list_pointer_clone.lock().unwrap(),
                    &mut passenger_stops_passed_pointer_clone.lock().unwrap(),
                    &sender,
                    &bus_stop_number_clone.lock().unwrap(),
                    &mut potential_running_bus_count_clone.lock().unwrap(),
                );
                //println!("Bus {} updated", simulated_bus.bus_num);

                match update_option {
                    None => break,
                    Some(_) => {}
                }
            }

            // println!("passenger list length: {}", passenger_list.len())
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

    println!(
        "Average wait time between stops {}",
        total_extra_stops_waited as f64 / total_passengers as f64
    );
}
