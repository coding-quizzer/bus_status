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

#[derive(Clone, Debug, PartialEq, Eq)]
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
    fn enter_bus<'a, T: Iterator<Item = &'a Location> + std::fmt::Debug>(
        &self,
        bus: &mut Bus<'a, T>,
    ) {
        let new_passenger = PassengerOnBus {
            id: self.id,
            end_location: self.end_location,
        };

        bus.add_passenger(new_passenger)
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
enum BusStatus {
    #[default]
    Moving,

    Stopped {
        location: Location,
    },
}

#[derive(Debug, Clone)]
struct Bus<'a, T>
where
    T: Iterator<Item = &'a Location> + std::fmt::Debug,
{
    unloading: bool,
    status: BusStatus,
    passengers: Vec<PassengerOnBus>,
    next_location: Location,
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
            unloading: false,
            status: BusStatus::Moving,
            passengers: vec![],
            next_location: Location::Loc1,
            current_location: None,
            location_list: iter,
        }
    }

    fn stop_at_location(&mut self, loc: Location) -> &mut Self {
        self.status = BusStatus::Stopped {
            location: self.next_location,
        };
        self
    }

    fn add_passenger(&mut self, passenger: PassengerOnBus) {
        self.passengers.push(passenger);
    }
}

fn location_from_index(index: u32) -> Option<Location> {
    match index {
        1 => Some(Location::Loc1),
        2 => Some(Location::Loc2),
        3 => Some(Location::Loc3),
        4 => Some(Location::Loc4),
        _ => None,
    }
}

fn generate_passenger() -> PassengerWaiting {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let rand_value: u32 = rng.gen_range(1..=4);
    let old_location = location_from_index(rand_value).expect("Invalid index for old_location");

    let mut rand_value_2 = rng.gen_range(1..=4);

    while rand_value == rand_value_2 {
        rand_value_2 = rng.gen_range(1..=4);
    }

    let new_location = location_from_index(rand_value_2).expect("Invalid index for new_location");

    Passenger::new(old_location, new_location)
}

fn generate_passenger_list(count: u32) -> Vec<PassengerWaiting> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger())
    }

    passenger_list
}

fn loop_through_passenger_list<'a, T>(bus: &mut Bus<'a, T>, passenger_list: Vec<PassengerWaiting>)
where
    T: Iterator<Item = &'a Location> + std::fmt::Debug,
{
    for passenger in passenger_list {
        // if let Some(loc) = bus.current_location && loc == passenger.current_location {
        //     passenger.enter_bus(bus);
        // }
        if bus.status == BusStatus::Moving && bus.unloading == false {
            bus.current_location.map_or((), |loc| {
                if loc == passenger.current_location {
                    passenger.enter_bus(bus);
                    println!("Passenger {passenger:?} entered the bus");
                }
            })
        }
    }
}
fn main() {
    let location_vector = vec![
        Location::Loc1,
        Location::Loc2,
        Location::Loc3,
        Location::Loc4,
    ];

    let iter = location_vector.iter().cycle().take(10);
    let mut bus = Bus::new(iter);
    let passenger_list = generate_passenger_list(10);
    loop_through_passenger_list(&mut bus, passenger_list);
}
