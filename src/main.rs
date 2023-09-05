use std::{
    sync::{Arc, Mutex},
    thread,
};
use uuid::Uuid;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum Location {
    #[default]
    Loc1,
    Loc2,
    Loc3,
    Loc4,
    Loc5,
    Loc6,
    Loc7,
    Loc8,
    Loc9,
    Loc10,
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
    fn new(current_location: Location, end_location: Location) -> PassengerWaiting {
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
    only_unloading: bool,
    movement: MovementState,
}

struct Bus {
    // unloading: bool,
    status: BusStatus,
    passengers: Vec<PassengerOnBus>,
    current_location: Option<Location>,
    location_iter: Box<dyn Iterator<Item = Location>>,
    location_vec: Vec<Location>,
    capacity: usize,
}

// manually impliment Debug, so that the iterator field can be skipped, eliminating the complicaiton of requiring
// the iterator to impliment Debug
impl std::fmt::Debug for Bus {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        f.debug_struct("Bus")
            .field("status", &self.status)
            .field("passengers", &self.passengers)
            .field("current_location", &self.current_location)
            .field("location_vec", &&self.location_vec)
            .finish()
    }
}
impl Bus {
    fn new(location_vector: Vec<Location>, capacity: usize, num_stops: usize) -> Bus {
        let location_vec = location_vector.clone();
        // Bus::default()
        let iterator = location_vector.into_iter().cycle().take(num_stops);
        Bus {
            status: BusStatus {
                only_unloading: false,
                movement: MovementState::Moving,
            },
            passengers: vec![],
            current_location: None,
            location_iter: Box::new(iterator),
            location_vec,
            capacity,
        }
    }

    fn set_offboarding_locations(&mut self) {
        let location_vec = self.location_vec.clone();

        let iterator = location_vec.into_iter();

        self.location_iter = Box::new(iterator);
    }

    fn stop_at_next_location(&mut self) -> Option<()> {
        self.current_location = self.location_iter.next();
        if let None = self.current_location {
            return None;
        }

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
            if let Some(_) = stop_output_option {
                return Some(());
            };

            if self.status.only_unloading == true {
                self.status.movement = MovementState::Finished;
                return None;
            }
            self.status.only_unloading = true;
            self.set_offboarding_locations();
        } else {
            self.drop_off_passengers(passenger_wait_list);

            if self.status.only_unloading == false {
                self.take_passengers(waiting_passengers);
            }
            self.status.movement = MovementState::Moving;
        }
        Some(())
    }

    fn take_passengers(&mut self, waiting_passengers: &mut Vec<PassengerWaiting>) {
        let mut new_passengers = vec![];
        for passenger in &mut *waiting_passengers {
            if &self.passengers.len() == &self.capacity {
                break;
            }
            // TODO: rewrite to check if future
            self.current_location.map_or((), |loc| {
                if loc == passenger.current_location {
                    let onboard_passenger = passenger.convert_to_onboarded_passenger();
                    self.add_passenger(&onboard_passenger);
                    new_passengers.push(passenger.clone());
                }
            })
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
                passenger_passed_stops.push(pass.passed_stops)
            } else {
                pass.passed_stops += 1;
                new_bus_passengers.push(pass.clone());
            }
        }
        self.passengers = new_bus_passengers;
        Some(())
    }
}

fn generate_passenger(location_list: &Vec<Location>) -> Result<PassengerWaiting, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let max_range = location_list.len();
    if max_range < 2 {
        return Err(format!(
            "Invalid Vec Length: {}. Length must be at least 2",
            max_range
        ));
    }
    let rand_value = rng.gen_range(0..max_range);
    let old_location = location_list
        .get(rand_value)
        .expect(format!("Invalid index for old_location: {rand_value}").as_str());

    let mut rand_value_2 = rng.gen_range(0..max_range);

    while rand_value == rand_value_2 {
        rand_value_2 = rng.gen_range(0..max_range);
    }

    let new_location = location_list
        .get(rand_value_2)
        .expect(format!("Invalid index for new_location: {rand_value_2}").as_str());

    Ok(Passenger::new(*old_location, *new_location))
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

const GLOBAL_PASSENGER_COUNT: u32 = 500;
const BUS_CAPACITY: usize = 10;
const NUM_OF_BUSES: u32 = 4;
const NUM_STOPS_PER_BUS: usize = 10;
fn main() {
    let location_vector = vec![
        Location::Loc1,
        Location::Loc2,
        Location::Loc3,
        Location::Loc4,
        Location::Loc5,
        Location::Loc6,
        Location::Loc7,
        Location::Loc8,
        Location::Loc9,
        Location::Loc10,
    ];
    let passenger_list_pointer = Arc::new(Mutex::new(
        generate_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap(),
    ));

    let location_vector_arc = Arc::new(location_vector);

    let passenger_wait_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    for _ in 1..=NUM_OF_BUSES {
        let bus_location_arc = location_vector_arc.clone();
        let passenger_list_pointer_clone = passenger_list_pointer.clone();
        let passenger_wait_pointer_clone = passenger_wait_pointer.clone();
        let handle = thread::spawn(move || {
            let bus_location_vector = bus_location_arc.as_ref();
            let mut passenger_list = passenger_list_pointer_clone.lock().unwrap();
            // let bus_location_vector = Arc::into_inner(bus_location_arc).unwrap();
            let mut simulated_bus =
                Bus::new(bus_location_vector.clone(), BUS_CAPACITY, NUM_STOPS_PER_BUS);
            let mut passenger_wait_list = passenger_wait_pointer_clone.lock().unwrap();

            loop {
                let update_option =
                    simulated_bus.update(&mut *passenger_list, &mut *passenger_wait_list);

                match update_option {
                    None => break,
                    Some(_) => {}
                }
            }
            if !passenger_wait_list.is_empty() {
                let passenger_count = passenger_wait_list.len();
                println!(
                    "Passenger wait average: {:?}",
                    &(*passenger_wait_list).iter().sum::<u32>().into() / passenger_count as f64
                );
            }
        });

        handle.join().unwrap();
    }
}
