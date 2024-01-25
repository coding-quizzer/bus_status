use crate::bus::SendableBus;
use crate::calculate_passenger_schedule_for_bus;
use crate::location::{Location, PassengerBusLocation};
use crate::passenger::PassengerOnboardingBusSchedule;
use crate::passenger::{Passenger, PassengerInfo};

pub struct PassengerScheduleWithDistance {
    pub passenger_schedule: VecDeque<PassengerOnboardingBusSchedule>,
    pub distance: u32,
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

use std::collections::VecDeque;

#[derive(Debug)]
pub struct Station {
    pub location: Location,
    pub docked_buses: Vec<SendableBus>,
    pub passengers: Vec<PassengerInfo>,
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
        mut new_passenger_info: PassengerInfo,
        time_tick: u32,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    ) -> Result<(), Passenger> {
        let mut new_passenger = new_passenger_info.passenger;
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule =
            calculate_passenger_schedule_for_bus(&new_passenger, time_tick, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule = new_bus_schedule;
        new_passenger_info.passenger = new_passenger;
        self.passengers.push(new_passenger_info);
        Ok(())
    }
}
