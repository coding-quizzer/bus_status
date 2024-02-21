use crate::bus::SendableBus;
use crate::calculate_passenger_schedule_for_bus;
use crate::location::{Location, PassengerBusLocation};
use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;

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
        new_passenger: Passenger,
        current_time_tick: u32,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
        buses_unavailable: Vec<usize>,
    ) -> Result<(), Passenger> {
        let mut new_bus_route_list = bus_route_list.clone();

        // mutable iterator containing the routes that will need to remove the
        let special_routes_iter =
            new_bus_route_list
                .iter_mut()
                .enumerate()
                .filter(|(bus_index, bus_route)| {
                    buses_unavailable.iter().any(|bus: &usize| bus == bus_index)
                });

        for (_, route) in special_routes_iter {
            let mut index_to_delete: Option<usize> = None;
            for (index_in_route, location) in route.iter().enumerate() {
                if current_time_tick == location.location_time_tick {
                    index_to_delete = Some(index_in_route);
                    break;
                }
            }

            if let Some(index) = index_to_delete {
                route.remove(index);
            }
        }

        // Take bus routes with the same time tick in unavailable buses out

        self.add_passenger(new_passenger, current_time_tick, &new_bus_route_list)
    }
}
