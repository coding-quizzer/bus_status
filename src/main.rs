use std::{
    sync::{Arc, Mutex},
    thread,
};
use uuid::Uuid;

fn generate_list_of_random_elements_from_list<T: Copy>(
    list: &Vec<T>,
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
    fn convert_to_onboarded_passenger(&self) -> PassengerOnBus {
        PassengerOnBus {
            id: self.id,
            end_location: self.end_location,
            passed_stops: 0,
        }
    }
}

struct Passenger;
impl Passenger {
    fn init(current_location: Location, end_location: Location) -> PassengerWaiting {
        PassengerWaiting {
            id: Uuid::new_v4(),
            current_location,
            end_location,
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
impl Bus {
    fn new(location_vector: Vec<Location>, capacity: usize) -> Bus {
        let location_vec = location_vector.clone();
        let iterator = location_vector.into_iter();
        Bus {
            status: BusStatus {
                movement: MovementState::Moving,
            },
            passengers: vec![],
            current_location: None,
            bus_route_iter: Box::new(iterator),
            bus_route_vec: location_vec,
            capacity,
            total_passenger_count: 0,
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
        passenger_wait_list: &mut Vec<u32>,
    ) -> Option<()> {
        if self.status.movement == MovementState::Moving {
            let stop_output_option = self.stop_at_next_location();
            if stop_output_option.is_some() {
                return Some(());
            };
            assert_eq!(self.passengers.len(), 0);
            self.status.movement = MovementState::Finished;
            return None;
        } else {
            self.drop_off_passengers(passenger_wait_list);
            self.take_passengers(waiting_passengers);
            self.status.movement = MovementState::Moving;
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
        panic!("Returnedd Vector was invalid")
    };

    Ok(Passenger::init(old_location, new_location))
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

    let passenger_wait_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    let mut handle_list = vec![];

    for _ in 1..=NUM_OF_BUSES {
        let bus_route =
            generate_bus_route(location_vector_arc.clone().as_ref(), NUM_STOPS_PER_BUS).unwrap();
        let passenger_list_pointer_clone = passenger_list_pointer.clone();
        let passenger_wait_pointer_clone = passenger_wait_pointer.clone();
        let handle = thread::spawn(move || {
            let mut passenger_list = passenger_list_pointer_clone.lock().unwrap();
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY);
            let mut passenger_wait_list = passenger_wait_pointer_clone.lock().unwrap();

            loop {
                let update_option =
                    simulated_bus.update(&mut passenger_list, &mut passenger_wait_list);

                match update_option {
                    None => break,
                    Some(_) => {}
                }
            }
            if !passenger_wait_list.is_empty() {
                let passenger_count = passenger_wait_list.len();
                println!(
                    "Bus Passenger count: {:?}",
                    simulated_bus.total_passenger_count
                );
                println!(
                    "Passenger wait average: {:?}",
                    Into::<f64>::into((*passenger_wait_list).iter().sum::<u32>())
                        / passenger_count as f64
                );
            }
            // println!("passenger list length: {}", passenger_list.len())
        });
        handle_list.push(handle);
    }
    for handle in handle_list {
        handle.join().unwrap();
    }
}
