use crate::bus::SendableBus;
use crate::location::{Location, PassengerBusLocation};
use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;
use crate::{
    calculate_passenger_schedule_for_bus,
    calculate_passenger_schedule_for_bus_check_available_buses,
};

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
    pub passengers: Vec<Passenger>,
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
    // I might have done it - clarify
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
            calculate_passenger_schedule_for_bus(&new_passenger, time_tick, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule_iterator = new_bus_schedule.clone().into_iter().peekable();
        new_passenger.bus_schedule = new_bus_schedule;
        self.passengers.push(new_passenger);
        Ok(())
    }

    pub fn add_passenger_check_available_buses(
        &mut self,
        mut new_passenger: Passenger,
        current_time_tick: u32,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
        buses_unavailable: Vec<usize>,
    ) -> Result<(), Passenger> {
        // TODO: Add tests to prove that this method of removing those indeces actually work
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule = calculate_passenger_schedule_for_bus_check_available_buses(
            &new_passenger,
            current_time_tick,
            bus_route_list,
            buses_unavailable,
        )?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule_iterator = new_bus_schedule.clone().into_iter().peekable();
        new_passenger.bus_schedule = new_bus_schedule;
        self.passengers.push(new_passenger);
        Ok(())
    }
}
