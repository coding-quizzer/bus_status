use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;
use crate::station::Station;
use crate::thread::SyncToBusMessages;
use crate::thread::{BusMessages, StationEventMessages, StationToBusMessages};
use crate::TimeTick;
use crate::TimeTickStage;
use core::time;
use std::thread::current;
use std::vec;
// use bus_system::Location;
pub use crate::location::BusLocation;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use std::sync::mpsc::{Receiver, Sender};

use crate::location::Location;

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
pub trait CloneIterator: Iterator + Send {
    fn clone_box(&self) -> Box<dyn CloneIterator<Item = Self::Item>>;
}

impl<T> Clone for Box<dyn CloneIterator<Item = T>>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

impl<T> CloneIterator for T
where
    T: Clone + Iterator + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneIterator<Item = T::Item>> {
        Box::new(self.clone())
    }
}

// where
//     T: 'static + Iterator + Clone + Send,
// {
//     fn clone_box(&self) -> Box<dyn CloneIterator<Item = Self::Item>> {
//         Box::new(self.clone())
//     }
// }

// impl<T> Send for dyn CloneIterator<Item = T> where T: Send {}

// Evantually, Passengers will need to somehow access the bus timetable to determine what bus will reach
// their destination. So far, the bus has decided what passengers can get on, but at this point
// the passengers should probably choose.

// The current bus route data the buses impliment may not be the same format the passengers would want
// It's hard to compare the positions of the buses at each time tick.s

#[derive(Clone, Debug)]
pub struct Bus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_iter: vec::IntoIter<BusLocation>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    // number of passengers
    numbers_of_passengers_serviced: u32,
    // TODO: propigate
    pub time_tick: TimeTick,
    pub bus_index: usize,
}

enum UpdateOutput {
    // WrongTimeTick,
    MovingBus,
    ReceivedPassengers { rejected_passengers: Vec<Passenger> },
}

impl Bus {
    pub fn try_new(bus_route: Vec<BusLocation>, capacity: usize, bus_num: usize) -> Option<Bus> {
        let bus_route_vec = bus_route.clone();
        let mut iterator = bus_route.into_iter();
        println!("Calling Bus::new from bus {}", bus_num);
        println!("bus route iterator: {:?}", iterator);
        let first_bus_location = iterator.next()?;
        let BusLocation {
            location: first_bus_location,
            distance_to_location: distance_to_first_location,
        } = first_bus_location;
        Some(Bus {
            status: BusStatus {
                movement: MovementState::Moving(distance_to_first_location),
            },
            passengers: vec![],
            current_location: Some(first_bus_location),
            bus_route_iter: iterator,
            bus_route_vec,
            capacity,
            numbers_of_passengers_serviced: 0,
            time_tick: TimeTick::default(),
            bus_index: bus_num,
        })
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

    // FIXME: same passenger is sometimes added to the bus twice. Why?
    pub fn add_passenger(&mut self, passenger: &Passenger) {
        // Ensure the passenger is not already on the bus
        assert!(
            !(&self
                .passengers
                .iter()
                .any(|bus_passenger| bus_passenger == passenger))
        );
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
        station_senders: &[Sender<StationEventMessages>],
        station_receiver: &Receiver<StationToBusMessages>,
        sync_sender: &Sender<BusMessages>,
        sync_receiver: &Receiver<SyncToBusMessages>,
    ) -> ControlFlow<()> {
        // somewhere there is a time advane message that is not recorded
        println!(
            "Bus {} update function current time tick: {:?}",
            self.bus_index, self.time_tick
        );
        println!("Bus movement: {:?}", self.status.movement);
        if let MovementState::Moving(distance) = self.status.movement {
            println!("Moving bus update");
            // println!(
            //     "Bus {} Moving on time tick {}",
            //     self.bus_num, current_time_tick_number
            // );

            // Only manage movement state during bus_unloading_passengers stage so that the bus only moves once
            // if let TimeTickStage::BusLoadingPassengers { .. } = self.time_tick.stage {
            //     return ControlFlow::Continue(());
            // }
            if distance > 1 {
                println!("Bus {} distance to next stop: {}", self.bus_index, distance);
                self.status.movement = MovementState::Moving(distance - 1);
                // return Some(());
            } else {
                println!("Bus {}, will stop at next time tick", self.bus_index);
                self.stop_at_destination_stop();
            }
            // self.time_tick_num += 1;
            sync_sender
                .send(BusMessages::AdvanceTimeStepForMovingBus {
                    //current_time_step: self.time_tick_num,
                    bus_index: self.bus_index,
                })
                .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_index, error));
            println!(
                "Bus {} sent message to sync thread for moving bus",
                self.bus_index
            );
            return ControlFlow::Continue(());
        } else {
            let current_location_index = self.current_location.unwrap().index;
            let next_station_sender = &station_senders[current_location_index];
            let current_location = self.current_location.unwrap();
            let mut passenger_current_location_indeces = Vec::new();
            let mut current_passenger_location_index: usize = 0;
            // Remove passengers getting off at

            // TODO: Make into a sub function
            let (outgoing_passengers, remaining_passengers): (Vec<_>, Vec<_>) =
                self.passengers.clone().into_iter().partition(|passenger| {
                    let is_offboarding = passenger
                        .bus_schedule
                        .clone()
                        .iter()
                        .enumerate()
                        // the schedules should only include any location once, so if this location comes up,
                        .any(|(location_index, passenger_location)| {
                            // FIX: find a better way to get the list of location indeces that doesn't involve misusing any
                            current_passenger_location_index = location_index;
                            passenger_location.stop_location == current_location
                                && passenger_location.time_tick >= self.time_tick.number
                        });

                    println!("Is Offboarding: {is_offboarding}");

                    if is_offboarding {
                        passenger_current_location_indeces.push(current_passenger_location_index);
                    }
                    is_offboarding
                });

            println!(
                "Bus {} Outgoing Passengers: {:#?}",
                self.bus_index, outgoing_passengers
            );
            println!(
                "Bus {} Remaining Passengers: {:#?}",
                self.bus_index, remaining_passengers
            );
            // This is a checks for an implimentation detail rather than the system as a whold
            debug_assert_eq!(
                outgoing_passengers.len(),
                passenger_current_location_indeces.len(),
                "List of indeces for next locations of valid passengers must be the same length as valid passengers"
            );

            // Passenger

            // let passenger_info_list: Vec<_> =
            //     std::iter::zip(outgoing_passengers, passenger_current_location_indeces)
            //         .map(|(passenger, location_index)| PassengerInfo {
            //             current_location_index: location_index,
            //             passenger,
            //         })
            //         .collect();

            self.passengers = remaining_passengers;
            println!(
                "New Passengers list for Bus {}: {:#?}",
                self.bus_index, self.passengers
            );
            let current_passenger_count = self.passengers.len();
            let capacity_remaining = self.capacity - current_passenger_count;
            let bus_info_for_station = SendableBus {
                capacity_remaining,
                bus_index: self.bus_index,
            };

            // outgoing_passengers is always empty. Why?

            println!(
                "Bus {} sending Passengers: {:?}",
                self.bus_index, outgoing_passengers
            );

            next_station_sender
                .send(StationEventMessages::BusArrived {
                    passengers_onboarding: outgoing_passengers,
                    bus_info: bus_info_for_station,
                })
                .unwrap();
            println!(
                "Bus {} arrived at station {}. Sent message",
                self.bus_index, current_location_index
            );

            let mut bus_departed = false;

            // DEBUG: this is not allowing the bus to transition from the bus unloading logic to the bus loading logic

            // command for bus departing is happening before
            while !bus_departed {
                println!("Waiting for station messages");
                let received_message = station_receiver.recv().unwrap();
                match received_message {
                    StationToBusMessages::AcknowledgeArrival() => {
                        println!("Bus {} acknowledgement received", self.bus_index);
                    }
                    StationToBusMessages::RequestDeparture => {
                        // dbg!(time_tick);
                        if let TimeTickStage::BusLoadingPassengers { .. } = self.time_tick.stage {
                        } else {
                            println!("Debug: waiting for time step to advance in bus");
                            // TODO: Replace with waiting for time tick
                            let incoming_message = sync_receiver.recv().unwrap();

                            let time_tick_temp: TimeTick = match incoming_message {
                                SyncToBusMessages::AdvanceTimeStep(time_step) => time_step,
                                // So far, there are no other options
                                _ => unreachable!(),
                            };
                            println!(
                                "Bus {} Time tick incremented. Time tick: {time_tick_temp:?}",
                                self.bus_index
                            );
                            self.time_tick = time_tick_temp;
                            println!(
                                "Time tick incremented self. Time tick: {:?}",
                                self.time_tick
                            );
                        };

                        next_station_sender
                            .send(StationEventMessages::BusDeparted {
                                bus_index: self.bus_index,
                            })
                            .unwrap();

                        println!(
                            "Bus {} request departure recieved from station {} and sent again on bus time tick {:?}",
                            self.bus_index,
                            current_location_index,
                            self.time_tick,

                        );
                    }
                    StationToBusMessages::SendPassengers(passenger_list) => {
                        println!(
                            "{} passengers added to bus {}",
                            passenger_list.len(),
                            self.bus_index
                        );
                        for passenger in passenger_list.iter() {
                            self.add_passenger(passenger);
                        }
                    }
                    StationToBusMessages::FinishedUnloading => {
                        println!(
                            "Bus {} unloading message received in bus from station",
                            self.bus_index
                        );
                        // This message is not always sent
                        sync_sender
                            .send(BusMessages::AdvanceTimeStepForUnloadedBus {
                                bus_index: self.bus_index,
                            })
                            .unwrap_or_else(|error| {
                                panic!("Error from bus {}: {}", self.bus_index, error)
                            });
                    }
                    StationToBusMessages::StationRemovedBus => {
                        println!(
                            "StationRemovedBus message received from station. Bus number: {}",
                            self.bus_index
                        );
                        sync_sender
                            .send(BusMessages::AdvanceTimeStepForLoadedBus {
                                //current_time_step: self.time_tick_num,
                                bus_index: self.bus_index,
                            })
                            .unwrap_or_else(|error| {
                                panic!("Error from bus {}: {}", self.bus_index, error)
                            });

                        let leave_result = self.leave_for_next_location();
                        if leave_result.is_none() {
                            // FIXME: The bus still contains all the passengers that got on it. None have exited the bus,
                            // event though they should have on previous stops
                            // Bus should be empty
                            assert_eq!(
                                self.passengers.len(),
                                0,
                                "Bus {} has {} passengers remaining. It should be empty. Passengers: {:#?}",
                                self.bus_index,
                                self.passengers.len(),
                                self.passengers
                            );
                            println!("Bus number {} is finished", self.bus_index);
                            sync_sender
                                .send(BusMessages::BusFinished {
                                    bus_index: self.bus_index,
                                })
                                .unwrap();
                            self.status.movement = MovementState::Finished;
                            return ControlFlow::Break(());
                        }
                        bus_departed = true;
                    }
                }
            }
            ControlFlow::Continue(())
        }
    }

    pub fn take_passengers(
        &mut self,
        waiting_passengers: &mut [Passenger],
        time_tick: &u32,
    ) -> Vec<Passenger> {
        let mut overflow_passengers = vec![];
        for passenger in waiting_passengers.iter_mut() {
            // Don't take a passenger if the bus is full or the passenger is either already on a bus or at his destination
            // if self.passengers.len() >= self.capacity
            //     || passenger.status != PassengerStatus::Waiting

            let PassengerOnboardingBusSchedule {
                stop_location: _,
                time_tick: onboarding_time_tick,
                bus_num,
            } = passenger
                .bus_schedule
                .first()
                .expect("Passenger schedule cannot be empty");

            println!("Onboarding time tick: {onboarding_time_tick}.");
            println!("Current time tick: {time_tick}");
            if onboarding_time_tick == time_tick && bus_num.expect("At this point, this cannot be the last bus location, and thus the bus_num must exist") == self.bus_index {
              println!("This is the correct time tick and bus");
              if self.passengers.len() >= self.capacity {
                  println!("Passenger Rejected. Bus Overfull");
                  overflow_passengers.push(passenger.clone());
              } else {
                println!("Onboarded Passenger: {:#?}", passenger);
                self.add_passenger(passenger);
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
            self.bus_index, self.current_location
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
                println!("Passenger left Bus {}", self.bus_index);
                passenger_passed_stops.push(passenger.passed_stops);
                self.numbers_of_passengers_serviced += 1;
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
/* pub struct SendableBus {
  status: BusStatus,
  passengers: Vec<Passenger>,
  current_location: Option<Location>,
  bus_route_vec: Vec<BusLocation>,
  capacity: usize,
  total_passenger_count: u32,
  // time_tick_num: u32,
  pub bus_num: usize,
} */

#[derive(Debug, Clone, PartialEq)]
pub struct SendableBus {
    pub capacity_remaining: usize,
    pub bus_index: usize,
}

/* impl From<Bus> for SendableBus {
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
} */
