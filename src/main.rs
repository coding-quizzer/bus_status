#[allow(unused_imports)]
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
}

#[derive(Debug, Clone)]
struct BusStatus {
    unloading: bool,
    movement: MovementState,
}

#[derive(Debug, Clone)]
struct Bus<'a, T>
where
    T: Iterator<Item = &'a Location> + std::fmt::Debug,
{
    // unloading: bool,
    status: BusStatus,
    passengers: Vec<PassengerOnBus>,
    current_location: Option<Location>,
    location_list: T,
}

// enum BusState {
//     Moving,
//     Stopped,
//     Unloading,
//     MovingUnloading,
// }

impl<'a, T> Bus<'a, T>
where
    T: Iterator<Item = &'a Location> + std::fmt::Debug,
{
    fn new(iter: T) -> Bus<'a, T>
    where
        T: Iterator<Item = &'a Location>,
    {
        // Bus::default()
        Bus {
            status: BusStatus {
                unloading: false,
                movement: MovementState::Moving,
            },
            // TODO: change passenger_list into an fixed-size array, acting as a max bus capacity
            passengers: vec![],
            current_location: None,
            location_list: iter,
        }
    }

    fn stop_at_location(&mut self) -> Option<()> {
        self.current_location = self.location_list.next().copied();
        if let None = self.current_location {
            return None;
        }

        self.status.movement = MovementState::Stopped;
        Some(())
    }

    fn add_passenger(&mut self, passenger: &PassengerOnBus) {
        self.passengers.push(*passenger);
    }

    fn update(&mut self) {}

    fn take_passengers(&mut self, waiting_passengers: &mut Vec<PassengerWaiting>)
    where
        T: Iterator<Item = &'a Location> + std::fmt::Debug,
    {
        let mut new_passengers = vec![];
        for passenger in &mut *waiting_passengers {
            self.current_location.map_or((), |loc| {
                if loc == passenger.current_location {
                    let onboard_passenger = passenger.convert_to_onboarded_passenger();
                    self.add_passenger(&onboard_passenger);
                    new_passengers.push(passenger.clone());
                    // println!("Passenger {passenger:?} entered the bus");
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

fn main() {
    let location_vector = vec![
        Location::Loc1,
        Location::Loc2,
        Location::Loc3,
        Location::Loc4,
    ];

    let iter = location_vector.iter().cycle().take(10);
    let mut simulated_bus = Bus::new(iter);
    let mut passenger_list = generate_passenger_list(10, &location_vector);

    dbg!(&passenger_list);
    loop {
        println!("____________________Next Stop________________");
        match simulated_bus.stop_at_location() {
            Some(()) => {}
            None => break,
        }
        simulated_bus.take_passengers(&mut passenger_list);
        dbg!(&simulated_bus.current_location);
        dbg!(&passenger_list);
        dbg!(&simulated_bus.passengers);
        simulated_bus.drop_off_passengers();
    }
}
