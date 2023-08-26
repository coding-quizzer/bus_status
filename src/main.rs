use std::{sync::mpsc, thread};
use uuid::Uuid;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum Location {
    #[default]
    Loc1,
    Loc2,
    Loc3,
    Loc4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PassengerOnBus {
    id: Uuid,
    end_location: Location,
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
    unloading: bool,
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
    fn new(location_vector: Vec<Location>, capacity: usize) -> Bus {
        let location_vec = location_vector.clone();
        // Bus::default()
        let iterator = location_vector.into_iter().cycle().take(10);
        Bus {
            status: BusStatus {
                unloading: false,
                movement: MovementState::Moving,
            },
            passengers: vec![],
            current_location: None,
            location_iter: Box::new(iterator),
            location_vec,
            capacity,
        }
    }

    fn reset_locations(&mut self) {
        let location_vec = self.location_vec.clone();

        let iterator = location_vec.into_iter();

        self.location_iter = Box::new(iterator);
    }

    fn stop_at_next_location(&mut self) -> Option<()> {
        self.current_location = self.location_iter.next();
        // if let None = self.current_location {
        //     return None;
        // }

        self.status.movement = MovementState::Stopped;
        Some(())
    }

    fn add_passenger(&mut self, passenger: &PassengerOnBus) {
        self.passengers.push(*passenger);
    }

    fn update(&mut self, waiting_passengers: &mut Vec<PassengerWaiting>) -> Option<()> {
        dbg!(&self);
        if self.status.movement == MovementState::Moving {
            self.stop_at_next_location();
            if let Some(_) = self.current_location {
                return Some(());
            }
            if self.status.unloading == true {
                self.status.movement = MovementState::Finished;
                return None;
            }
            self.status.unloading = true;
            self.reset_locations();
        } else {
            if self.status.unloading == false {
                self.take_passengers(waiting_passengers);
            }
            dbg!(self.current_location);
            dbg!(waiting_passengers);
            dbg!(&self.passengers);
            self.drop_off_passengers();
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
    fn drop_off_passengers(&mut self) -> Option<()> {
        let current_location = self.current_location?;
        self.passengers
            .retain(|pass| pass.end_location != (current_location));
        Some(())
    }
}

fn generate_passenger(location_list: &Vec<Location>) -> PassengerWaiting {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let max_range = location_list.len();
    let rand_value = rng.gen_range(1..max_range);
    let old_location = location_list
        .get(rand_value)
        .expect(format!("Invalid index for old_location: {rand_value}").as_str());

    let mut rand_value_2 = rng.gen_range(0..4);

    while rand_value == rand_value_2 {
        rand_value_2 = rng.gen_range(0..4);
    }

    let new_location = location_list
        .get(rand_value_2)
        .expect(format!("Invalid index for new_location: {rand_value_2}").as_str());

    Passenger::new(*old_location, *new_location)
}

fn generate_passenger_list(count: u32, location_list: &Vec<Location>) -> Vec<PassengerWaiting> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger(location_list))
    }

    passenger_list
}

const GLOBAL_PASSENGER_COUNT: u32 = 30;
const BUS_CAPACITY: usize = 20;
fn main() {
    let location_vector = vec![
        Location::Loc1,
        Location::Loc2,
        Location::Loc3,
        Location::Loc4,
    ];

    let mut passenger_list = generate_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector);

    let bus_location_vector = location_vector.clone();
    let handle = thread::spawn(move || {
        let mut simulated_bus = Bus::new(bus_location_vector, BUS_CAPACITY);

        dbg!(&passenger_list);
        loop {
            println!("____________________Next Stop________________");
            let update_option = simulated_bus.update(&mut passenger_list);
            match update_option {
                None => break,
                Some(_) => {}
            }
        }
    });

    handle.join().unwrap();
}
