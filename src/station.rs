use crate::bus::SendableBus;
use crate::location::{Location, PassengerBusLocation};
use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;

struct PassengerScheduleWithDistance {
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

fn calculate_passenger_schedule_for_bus(
    passenger: Passenger,
    current_time_tick: u32,
    bus_route_list: &Vec<Vec<PassengerBusLocation>>,
) -> Result<Vec<PassengerOnboardingBusSchedule>, Passenger> {
    calculate_passenger_schedule_for_bus_with_recursion(
        passenger.current_location.unwrap(),
        passenger.destination_location,
        None,
        current_time_tick,
        bus_route_list,
        Vec::new(),
        None,
    )
    .map(|schedule| schedule.into())
    .ok_or(passenger)
}

fn calculate_passenger_schedule_for_bus_with_recursion(
    initial_location: Location,
    destination_location: Location,
    destination_time_tick: Option<u32>,
    current_time_tick: u32,
    bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    visited_locations: Vec<Location>,
    next_bus_index: Option<usize>,
) -> Option<VecDeque<PassengerOnboardingBusSchedule>> {
    let mut valid_schedules: Vec<PassengerScheduleWithDistance> = Vec::new();
    let mut destination_list = Vec::new();
    // let mut bus_schedule: VecDeque<PassengerOnboardingBusSchedule> = VecDeque::new();

    // Find the index of the bus route containing the destination location and the bus location containting the location and the time tick
    for (bus_index, bus_route) in bus_route_list.iter().enumerate() {
        for bus_location in bus_route {
            if bus_location.location == destination_location
                && current_time_tick <= bus_location.location_time_tick
                // The new destination is not identical to the old destination
                && !((next_bus_index.is_some() && (next_bus_index.unwrap() == bus_index))
                    && (destination_time_tick.is_some()
                        && destination_time_tick.unwrap() == bus_location.location_time_tick))
                && (destination_time_tick.is_none()
                    || destination_time_tick.unwrap() >= bus_location.location_time_tick)
            {
                destination_list.push((bus_index, bus_location));

                /* bus schedule should not contain both potential destinations */
                // bus_schedule.push_front(PassengerOnboardingBusSchedule {
                //     time_tick: bus_location.location_time_tick,
                //     bus_num: Some(bus_index),
                // });
            }
        }
    }

    for destination in destination_list.iter() {
        let mut bus_schedule = VecDeque::new();
        let mut visited_locations = visited_locations.clone();

        // let (destination_bus_index, destination_passenger_bus_location) = destination;
        // let trial_destination_time_tick = destination_passenger_bus_location.location_time_tick;
        // bus_schedule.push_front(PassengerOnboardingBusSchedule {
        //     stop_location: destination_passenger_bus_location.location,
        //     time_tick: trial_destination_time_tick,
        //     bus_num: next_bus_index,
        // });
        let (destination_bus_index, destination_passenger_bus_location) = destination;
        let trial_destination_time_tick = destination_passenger_bus_location.location_time_tick;

        bus_schedule.push_front(PassengerOnboardingBusSchedule {
            stop_location: destination_passenger_bus_location.location,
            time_tick: trial_destination_time_tick,
            bus_num: next_bus_index,
        });
        visited_locations.push(destination_passenger_bus_location.location);
        // Currently, current_bus_index and destination_bus_index mean the same thing
        // let current_bus_index = destination.0;
        let bus_route_for_destination_bus = &bus_route_list[*destination_bus_index];

        for bus_location in bus_route_for_destination_bus {
            // exclude the destination location just added
            #[allow(unused_parens)]
            if bus_location.location_time_tick <= trial_destination_time_tick
                && (destination_time_tick.is_none()
                    || (destination_time_tick.unwrap() >= current_time_tick))
                && (bus_location.location == initial_location)
            {
                let mut final_bus_schedule = bus_schedule.clone();

                final_bus_schedule.push_front(PassengerOnboardingBusSchedule {
                    time_tick: bus_location.location_time_tick,
                    stop_location: bus_location.location,
                    bus_num: Some(*destination_bus_index),
                });

                valid_schedules.push(final_bus_schedule.into());

                continue;
            } else if ((bus_location.location_time_tick < destination_passenger_bus_location.location_time_tick)
                    &&
                    // exclude locations in visited locations
                    !visited_locations
                        .clone()
                        .iter()
                        .any(|location| -> bool { location == &bus_location.location }))
            {
                let extension_bus_route = calculate_passenger_schedule_for_bus_with_recursion(
                    initial_location,
                    bus_location.location,
                    Some(bus_location.location_time_tick),
                    current_time_tick,
                    bus_route_list,
                    visited_locations.clone(),
                    Some(*destination_bus_index),
                );

                match extension_bus_route {
                    Some(mut bus_route) => {
                        // Tack the destination location to the end of the returned list
                        bus_route.push_back(PassengerOnboardingBusSchedule {
                            time_tick: destination_passenger_bus_location.location_time_tick,
                            stop_location: destination_passenger_bus_location.location,
                            bus_num: next_bus_index,
                        });
                        valid_schedules.push(bus_route.into());
                        continue;
                    }
                    None => continue,
                }
            }
        }
    }
    if valid_schedules.is_empty() {
        None
    } else {
        let optimal_schedule = valid_schedules
            .into_iter()
            .min_by(|trial_schedule, accum| (trial_schedule.distance).cmp(&accum.distance))
            .expect("Schedule must contain elements at this point");

        Some(optimal_schedule.passenger_schedule)
    }
}

#[derive(Debug)]
pub struct Station {
    pub location: Location,
    pub docked_buses: Vec<SendableBus>,
    passengers: Vec<Passenger>,
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
            calculate_passenger_schedule_for_bus(new_passenger.clone(), time_tick, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule = new_bus_schedule;
        self.passengers.push(new_passenger);
        Ok(())
    }
}
