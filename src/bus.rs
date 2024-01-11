
use crate::Location;
use crate::{BusMessages, StationMessages};
use crate::{Passenger, PassengerOnboardingBusSchedule, PassengerStatus};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::Sender;

#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
enum MovementState {
    // Moving contains the distance to the next location
    Moving(u32),
    Stopped,
    Finished,
}

impl Default for MovementState {
    fn default() -> Self {
        Self::Moving(0)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

// Evantually, Passengers will need to somehow access the bus timetable to determine what bus will reach
// their destination. So far, the bus has decided what passengers can get on, but at this point
// the passengers should probably choose.

// The current bus route data the buses impliment may not be the same format the passengers would want
// It's hard to compare the positions of the buses at each time tick.s

pub struct Bus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_iter: Box<dyn CloneIterator<Item = BusLocation>>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    total_passenger_count: u32,
    // time_tick_num: u32,
    pub bus_num: usize,
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

impl Clone for Bus {
    fn clone(&self) -> Self {
        Bus {
            status: self.status.clone(),
            passengers: self.passengers.clone(),
            current_location: self.current_location.clone(),
            bus_route_iter: self.bus_route_iter.clone_box(),
            bus_route_vec: self.bus_route_vec.clone(),
            capacity: self.capacity.clone(),
            total_passenger_count: self.total_passenger_count.clone(),
            bus_num: self.bus_num.clone(),
        }
    }
}

// let passengers take the most efficient route

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BusLocation {
    pub location: Location,
    // distance_to_next is None for the last location
    pub distance_to_location: u32,
}

enum UpdateOutput {
    // WrongTimeTick,
    MovingBus,
    ReceivedPassengers { rejected_passengers: Vec<Passenger> },
}

impl Bus {
    pub fn new(bus_route: Vec<BusLocation>, capacity: usize, bus_num: usize) -> Bus {
        let bus_route_vec = bus_route.clone();
        let mut iterator = bus_route.into_iter();
        let first_bus_location = iterator
            .next()
            .expect("Bus route must contain at least one location");
        let BusLocation {
            location: first_bus_location,
            distance_to_location: distance_to_first_location,
        } = first_bus_location;
        Bus {
            status: BusStatus {
                movement: MovementState::Moving(distance_to_first_location),
            },
            passengers: vec![],
            current_location: Some(first_bus_location),
            bus_route_iter: Box::new(iterator),
            bus_route_vec,
            capacity,
            total_passenger_count: 0,
            // time_tick_num: 0,
            bus_num,
        }
    }

    pub fn stop_at_destination_stop(&mut self) -> Option<()> {
        self.status.movement = MovementState::Stopped;
        Some(())
    }

    pub fn leave_for_next_location(&mut self) -> Option<()> {
        // Return None if the previous location was the end of the list
        let next_location = self.bus_route_iter.next()?;
        self.current_location = Some(next_location.location);

        self.status.movement = MovementState::Moving(next_location.distance_to_location);
        Some(())
    }

    pub fn add_passenger(&mut self, passenger: &Passenger) {
        self.passengers.push(passenger.clone());
    }

    // Somehow, the passenger thread will need to keep some kind of track how many buses have sent their reject customers to the
    // passenger thread so that all the passengers are considiered. One way to do this could be to send thoses messages to the
    // sync thread which can pretty easily track how many buses to be concerned about and have the sync thread send the passengers to the
    // passenger thread

    /* fn update(
        &mut self,
        waiting_passengers: &mut [Passenger],
        passenger_stops_waited_list: &mut Vec<u32>,
        sender: &Sender<BusMessages>,
        current_time_tick_number: &u32,
    ) -> ControlFlow<(), UpdateOutput> {
        println!("Update Beginning");
        println!(
            "Bus update. current time tick: {}",
            current_time_tick_number
        );

        // This might cause problems, since it sends a message for increaseing the time step
        // before the operations have actually been performed. Hopefully keeping the timestep
        // locked until after this function shoudl help?

        // Bus message
        // Holding output in a variable can allow a single send directly before the return
        // sender should be right before the

        println!("Bus Number {} Sent", self.bus_num);
        if let MovementState::Moving(distance) = self.status.movement {
            println!(
                "Bus {} Moving on time tick {}",
                self.bus_num, current_time_tick_number
            );
            if distance > 1 {
                println!("Bus {} distance to next stop: {}", self.bus_num, distance);
                self.status.movement = MovementState::Moving(distance - 1);
                // return Some(());
            } else {
                println!("Bus {}, will stop at next time tick", self.bus_num);
                self.stop_at_destination_stop();
            }
            // self.time_tick_num += 1;
            sender
                .send(BusMessages::AdvanceTimeStep {
                    //current_time_step: self.time_tick_num,
                    bus_index: self.bus_num,
                })
                .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
            ControlFlow::Continue(UpdateOutput::MovingBus)
        } else {
            println!("Bus {} stopped", self.bus_num);

            println!("Waiting Passengers: {:#?}", waiting_passengers);

            self.drop_off_passengers(passenger_stops_waited_list);
            let rejected_passengers =
                self.take_passengers(waiting_passengers, current_time_tick_number);

            let more_locations_left = self.leave_for_next_location();
            // self.time_tick_num += 1;

            if more_locations_left.is_some() {
                sender
                    .send(BusMessages::AdvanceTimeStep {
                        //current_time_step: self.time_tick_num,
                        bus_index: self.bus_num,
                    })
                    .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
                return ControlFlow::Continue(UpdateOutput::ReceivedPassengers {
                    rejected_passengers,
                });
            };

            assert_eq!(self.passengers.len(), 0);
            println!("Bus number {} is finished", self.bus_num);
            self.status.movement = MovementState::Finished;
            ControlFlow::Break(())
        }
    } */

    pub fn update(
        &mut self,
        station_senders: &[Sender<StationMessages>],
        sync_sender: &Sender<BusMessages>,
        // current_time_tick_number: &u32,
    ) {
        if let MovementState::Moving(distance) = self.status.movement {
            // println!(
            //     "Bus {} Moving on time tick {}",
            //     self.bus_num, current_time_tick_number
            // );
            if distance > 1 {
                println!("Bus {} distance to next stop: {}", self.bus_num, distance);
                self.status.movement = MovementState::Moving(distance - 1);
                // return Some(());
            } else {
                println!("Bus {}, will stop at next time tick", self.bus_num);
                self.stop_at_destination_stop();
            }
            // self.time_tick_num += 1;
            sync_sender
                .send(BusMessages::AdvanceTimeStep {
                    //current_time_step: self.time_tick_num,
                    bus_index: self.bus_num,
                })
                .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
            return;
        } else {
            println!("Bus {} Arrived", self.bus_num);
            let current_location_index = self.current_location.unwrap().index;
            let next_station_sender = &station_senders[current_location_index];
            next_station_sender
                .send(StationMessages::BusArrived(self.clone().into()))
                .unwrap();
            println!("Arrived Message sent.");
        }
        assert_eq!(self.passengers.len(), 0);
        println!("Bus number {} is finished", self.bus_num);
        self.status.movement = MovementState::Finished;
    }

    pub fn take_passengers(
        &mut self,
        waiting_passengers: &mut [Passenger],
        current_time_tick: &u32,
    ) -> Vec<Passenger> {
        let mut overflow_passengers = vec![];
        for passenger in waiting_passengers.iter_mut() {
            // Don't take a passenger if the bus is full or the passenger is either already on a bus or at his destination
            // if self.passengers.len() >= self.capacity
            //     || passenger.status != PassengerStatus::Waiting
            if passenger.status != PassengerStatus::Waiting {
                println!("Waiting passenger");
                continue;
            }

            let PassengerOnboardingBusSchedule {
                stop_location: _,
                time_tick: onboarding_time_tick,
                bus_num,
            } = passenger
                .bus_schedule
                .get(0)
                .expect("Passenger schedule cannot be empty");

            println!("Onboarding time tick: {onboarding_time_tick}.");
            println!("Current time tick: {current_time_tick}");
            if onboarding_time_tick == current_time_tick && bus_num.expect("At this point, this cannot be the last bus loction, and thus the bus_num must exist") == self.bus_num {
              println!("This is the correct time tick and bus");
              if self.passengers.len() >= self.capacity {
                  println!("Passenger Rejected. Bus Overfull");
                  overflow_passengers.push(passenger.clone());
              } else {
                println!("Onboarded Passenger: {:#?}", passenger);
                let onboard_passenger = passenger.convert_to_onboarded_passenger();
                self.add_passenger(onboard_passenger);
              }
            }

            // println!("Passengers on the bus: {:#?}", self.passengers);

            // let mut cloned_locations = self.bus_route_iter.clone_box();

            // might become a seperate function call

            // Letting Passengers in will eventually move to Passenger side instead of Bus side
            // let bus_will_stop_at_passengers_location = cloned_locations
            //     .any(|location_of_bus| location_of_bus.location == passenger.destination_location);

            // if bus_will_stop_at_passengers_location {
            //     self.current_location.map_or((), |loc| {
            //         if loc == passenger.current_location.unwrap() {
            //             let onboard_passenger = passenger.clone().convert_to_onboarded_passenger();
            //             self.add_passenger(&onboard_passenger);
            //         }
            //     })
            // }
        }
        println! {"Full Passenger list: {:#?}", self.passengers};
        overflow_passengers
    }
    pub fn drop_off_passengers(&mut self, passenger_passed_stops: &mut Vec<u32>) -> Option<()> {
        println!("Drop off Passengers");
        println!(
            "Bus {} current location: {:?}",
            self.bus_num, self.current_location
        );
        println!(
            "Remaining iterator: {:#?}",
            self.bus_route_iter.clone_box().collect::<Vec<_>>()
        );
        // println!("Passengers on the bus: {:#?}", self.passengers);

        let current_location = self.current_location?;
        let bus_passengers = &mut *self.passengers;
        let mut new_bus_passengers = vec![];
        println!("Bus Passengers: {:#?}", bus_passengers);
        for passenger in bus_passengers {
            if passenger.destination_location == current_location {
                println!("Passenger left Bus {}", self.bus_num);
                passenger_passed_stops.push(passenger.passed_stops);
                passenger.status = PassengerStatus::Arrived;
                self.total_passenger_count += 1;
            } else {
                passenger.passed_stops += 1;
                new_bus_passengers.push(passenger.clone());
            }
        }
        self.passengers = new_bus_passengers;
        Some(())
    }

    pub fn get_bus_route(&self) -> Vec<BusLocation> {
        self.bus_route_vec.clone()
    }
}

/* #[derive(Debug, Serialize, Deserialize)]
struct SerializableBus {
    id: u32,
    destination: Location,
    current_location: Option<Location>,
} */
#[derive(Debug, Serialize, Deserialize)]
pub struct SendableBus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    total_passenger_count: u32,
    // time_tick_num: u32,
    pub bus_num: usize,
}

impl From<Bus> for SendableBus {
    fn from(bus: Bus) -> SendableBus {
        let Bus {
            status,
            passengers,
            current_location,
            bus_route_vec,
            capacity,
            total_passenger_count,
            bus_num,
            bus_route_iter: _bus_route_iter,
        } = bus;

        SendableBus {
            status,
            passengers,
            current_location,
            bus_route_vec,
            capacity,
            total_passenger_count,
            bus_num,
        }
    }
}
