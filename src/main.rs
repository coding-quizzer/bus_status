#[allow(unused_imports)]
use std::{sync::mpsc, thread};

#[derive(Clone, Copy, Default, Debug)]
enum Location {
    #[default]
    Loc1,
    Loc2,
    Loc3,
    Loc4,
}

#[derive(Clone, Debug)]
struct PassengerOnBus {
    end_location: Location,
}
struct PassengerWaiting {
    current_location: Location,
    end_location: Location,
}

impl PassengerWaiting {
    fn enter_bus<'a, T: Iterator<Item = &'a Location> + std::fmt::Debug>(
        &self,
        passenger: Passenger,
        bus: &mut Bus<'a, T>,
    ) {
        let new_passenger = PassengerOnBus {
            end_location: self.end_location,
        };

        bus.add_passenger(new_passenger)
    }
}

struct Passenger;

impl Passenger {
    fn new(current_location: Location, end_location: Location) -> PassengerWaiting {
        PassengerWaiting {
            current_location,
            end_location,
        }
    }
}

#[derive(Default, Debug)]
enum CurrentBusStatus {
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
    status: CurrentBusStatus,
    passengers: Vec<PassengerOnBus>,
    next_location: Location,
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
            status: CurrentBusStatus::Moving,
            passengers: vec![],
            next_location: Location::Loc1,
            location_list: iter,
        }
    }

    fn stop_at_location(&mut self, loc: Location) -> &mut Self {
        self.status = CurrentBusStatus::Stopped {
            location: self.next_location,
        };
        self
    }

    fn add_passenger(&mut self, passenger: PassengerOnBus) {
        self.passengers.push(passenger);
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
    let bus = Bus::new(iter);
}
