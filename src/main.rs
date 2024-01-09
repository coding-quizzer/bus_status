use serde::{de, ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::{
    collections::VecDeque,
    error::Error,
    ffi::FromVecWithNulError,
    fs::File,
    hash::{BuildHasher, Hash},
    io::{BufReader, BufWriter},
    ops::ControlFlow,
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};
use uuid::Uuid;

/// generates a random list from a set of elements such that
/// no two consecutive elements are identical.
fn generate_list_of_random_elements_from_list<T: Copy>(
    // List of elements from which list is generated
    list: &Vec<T>,
    // Number of elements desired in the
    output_length: usize,
) -> Result<Vec<T>, String> {
    use rand::Rng;
    if list.len() <= 1 {
        return Err("List must contain at least 2 elements.".to_string());
    };
    let mut output_list = vec![];
    let mut rng = rand::thread_rng();
    let location_count = list.len();

    let mut old_index = rng.gen_range(0..location_count);
    output_list.push(list[old_index]);

    // Start at one because the first location was pushed before the for loop
    for _ in 1..output_length {
        let mut new_index;
        new_index = rng.gen_range(0..location_count);
        while old_index == new_index {
            new_index = rng.gen_range(0..location_count);
        }
        output_list.push(list[new_index]);
        old_index = new_index;
    }

    Ok(output_list)
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
struct Location {
    index: usize,
    id: Uuid,
}

impl Location {
    fn new(index: usize) -> Location {
        Location {
            id: Uuid::new_v4(),
            index,
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Location {}", self.index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
enum PassengerStatus {
    OnBus,
    Waiting,
    Arrived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct PassengerOnboardingBusSchedule {
    time_tick: u32,
    // the last destination will not include a bus number because the passenger will be at his destination
    bus_num: Option<usize>,
    stop_location: Location,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SerializedPassenger {
    id: Uuid,
    destination_location: Location,
    current_location: Option<Location>,
}

impl From<SerializedPassenger> for Passenger {
    fn from(serialized_passenger_input: SerializedPassenger) -> Passenger {
        let SerializedPassenger {
            id,
            destination_location,
            current_location,
        } = serialized_passenger_input;

        Passenger {
            id,
            destination_location,
            status: PassengerStatus::Waiting,
            current_location,
            passed_stops: 0,
            bus_schedule: Vec::new(),
        }
    }
}

impl From<Passenger> for SerializedPassenger {
    fn from(passenger: Passenger) -> SerializedPassenger {
        let Passenger {
            id,
            destination_location,
            status: _status,
            current_location,
            passed_stops: _passed_stops,
            bus_schedule: _bus_schedule,
        } = passenger;

        SerializedPassenger {
            id,
            destination_location,
            current_location,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
// Passenger should eventually contain the index and the time tick for the bus
// I'm not sure how recalculating the route should work if the passenger doesn't get on the bus
// I wonder if some sort of priority system would be useful sometime
struct Passenger {
    id: Uuid,
    destination_location: Location,
    status: PassengerStatus,
    current_location: Option<Location>,
    passed_stops: u32,
    bus_schedule: Vec<PassengerOnboardingBusSchedule>,
}
impl Passenger {
    fn new(current_location: Location, destination_location: Location) -> Self {
        Self {
            id: Uuid::new_v4(),
            current_location: Some(current_location),
            destination_location,
            status: PassengerStatus::Waiting,
            passed_stops: 0,
            bus_schedule: Vec::new(),
        }
    }

    fn convert_to_onboarded_passenger(&mut self) -> &Self {
        if self.status == PassengerStatus::Waiting {
            self.status = PassengerStatus::OnBus;
            self.current_location = None;
        }
        self
    }
}

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

struct Bus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_iter: Box<dyn CloneIterator<Item = BusLocation>>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    total_passenger_count: u32,
    // time_tick_num: u32,
    bus_num: usize,
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
struct BusLocation {
    location: Location,
    // distance_to_next is None for the last location
    distance_to_location: u32,
}

#[derive(Debug, Clone)]
struct PassengerBusLocation {
    location: Location,
    location_time_tick: u32,
}
enum UpdateOutput {
    // WrongTimeTick,
    MovingBus,
    ReceivedPassengers { rejected_passengers: Vec<Passenger> },
}

impl Bus {
    fn new(bus_route: Vec<BusLocation>, capacity: usize, bus_num: usize) -> Bus {
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

    fn stop_at_destination_stop(&mut self) -> Option<()> {
        self.status.movement = MovementState::Stopped;
        Some(())
    }

    fn leave_for_next_location(&mut self) -> Option<()> {
        // Return None if the previous location was the end of the list
        let next_location = self.bus_route_iter.next()?;
        self.current_location = Some(next_location.location);

        self.status.movement = MovementState::Moving(next_location.distance_to_location);
        Some(())
    }

    fn add_passenger(&mut self, passenger: &Passenger) {
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

    fn update(
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

    fn take_passengers(
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
    fn drop_off_passengers(&mut self, passenger_passed_stops: &mut Vec<u32>) -> Option<()> {
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

    fn get_bus_route(&self) -> Vec<BusLocation> {
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
struct SendableBus {
    status: BusStatus,
    passengers: Vec<Passenger>,
    current_location: Option<Location>,
    bus_route_vec: Vec<BusLocation>,
    capacity: usize,
    total_passenger_count: u32,
    // time_tick_num: u32,
    bus_num: usize,
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

#[derive(Debug)]
struct Station {
    location: Location,
    docked_buses: Vec<SendableBus>,
    passengers: Vec<Passenger>,
}

impl Station {
    fn new(location: Location) -> Self {
        Station {
            location: location,
            docked_buses: Vec::new(),
            passengers: Vec::new(),
        }
    }

    // TODO: Convert to a function returning an Option, figure out
    fn add_passenger(
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

#[derive(PartialEq, Debug)]
enum RejectedPassengersMessages {
    MovingBus,
    StoppedBus { rejected_passengers: Vec<Passenger> },
    CompletedProcessing,
}
#[derive(Debug)]
enum StationMessages {
    InitPassengerList(Vec<Passenger>),
    BusArrived(SendableBus),
}

#[derive(Debug, PartialEq, Eq)]
enum StationToPassengersMessages {
    ConfirmInitPassengerList(usize),
}
enum StationToBusMessages {
    AcknowledgeArrival(),
}

#[derive(PartialEq, Debug)]
enum BusMessages {
    InitBus {
        bus_index: usize,
    },
    InitPassengers,
    AdvanceTimeStep {
        // current_time_step: u32,
        bus_index: usize,
    },
    BusFinished {
        bus_index: usize,
    },

    RejectedPassengers(RejectedPassengersMessages),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BusThreadStatus {
    Uninitialized,
    BusFinishedRoute,
    WaitingForTimeStep,
    CompletedTimeStep,
}

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

struct PassengerScheduleWithDistance {
    passenger_schedule: VecDeque<PassengerOnboardingBusSchedule>,
    distance: u32,
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

fn generate_passenger(location_list: &Vec<Location>) -> Result<Passenger, String> {
    let location_vector = generate_list_of_random_elements_from_list(location_list, 2)?;

    let [old_location, new_location] = location_vector[..] else {
        panic!("Returned Vector was invalid")
    };

    Ok(Passenger::new(old_location, new_location))
}

fn generate_random_passenger_list(
    count: usize,
    location_list: &Vec<Location>,
) -> Result<Vec<Passenger>, String> {
    let mut passenger_list = vec![];
    for _num in 0..count {
        passenger_list.push(generate_passenger(location_list)?)
    }

    Ok(passenger_list)
}

fn convert_bus_route_list_to_passenger_bus_route_list(
    bus_route_list: Vec<BusLocation>,
) -> Vec<PassengerBusLocation> {
    let mut time_tick = 2;
    let mut passenger_bus_route_list = vec![];
    for bus_location in bus_route_list.iter() {
        let mut index_increment = 0;
        // If this is the not the first location, add two time ticks for waiting at the last station
        if time_tick != 2 {
            // Add one to the index_increment for the time tick used at the previous stop
            index_increment += 2;
        }

        // add time steps for the distance to the destination
        index_increment += bus_location.distance_to_location * 2;

        time_tick += index_increment;

        let passenger_bus_location = PassengerBusLocation {
            location: bus_location.location,
            location_time_tick: time_tick,
        };

        passenger_bus_route_list.push(passenger_bus_location);
    }

    passenger_bus_route_list
}

fn generate_bus_route_locations(
    location_list: &Vec<Location>,
    length: usize,
) -> Result<Vec<Location>, String> {
    let bus_route = generate_list_of_random_elements_from_list(location_list, length)?;

    Ok(bus_route)
}

fn generate_bus_route_locations_with_distances(
    location_list: &Vec<Location>,
    length: usize,
    max_distance: u32,
) -> Result<Vec<BusLocation>, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bus_route_list = generate_bus_route_locations(location_list, length);
    let bus_route_list_to_bus_location_types = bus_route_list?
        .iter()
        .map(|location| BusLocation {
            location: *location,
            distance_to_location: rng.gen_range(1..=max_distance),
        })
        .collect::<Vec<_>>();
    Ok(bus_route_list_to_bus_location_types)
}

// fn generate_bus_route_arr

fn initialize_location_list(count: usize) -> Vec<Location> {
    let mut location_list = vec![];
    for index in 0..count {
        location_list.push(Location::new(index));
    }
    location_list
}
fn initialize_channel_list<T>(channel_count: usize) -> (Vec<Sender<T>>, Vec<Receiver<T>>) {
    let mut sender_vector = Vec::new();
    let mut receiver_vector = Vec::new();
    for _ in 0..channel_count {
        let (current_sender, current_receiver) = mpsc::channel();
        sender_vector.push(current_sender);
        receiver_vector.push(current_receiver);
    }

    (sender_vector, receiver_vector)
}

#[derive(Clone, Serialize, Deserialize)]
struct InputDataStructure {
    bus_routes: [Vec<BusLocation>; NUM_OF_BUSES],
    passengers: Vec<SerializedPassenger>,
    location_vector: Vec<Location>,
}

/* struct ConvertedInputDataStructure {
  bus_routes: Vec<Vec<BusLocation>>,

} */

fn locations_are_all_valid(
    mut locations_to_test: impl Iterator<Item = Location>,
    location_master_list: &[Location],
) -> bool {
    // test every location
    !locations_to_test.any(|tested_location| {
        !location_master_list
            .iter()
            .any(|master_list| master_list == &tested_location)
    })
}

fn read_data_from_file(path: &Path) -> Result<InputDataStructure, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `Data`.
    let data: InputDataStructure = serde_json::from_reader(reader)?;

    let InputDataStructure {
        bus_routes,
        passengers,
        location_vector,
    } = data.clone();

    assert_eq!(location_vector.len(), GLOBAL_LOCATION_COUNT);
    assert_eq!(bus_routes.len(), NUM_OF_BUSES);
    assert_eq!(passengers.len(), GLOBAL_PASSENGER_COUNT);

    let bus_locations = bus_routes
        .iter()
        .flatten()
        .map(|bus_location| bus_location.location);
    // .collect::<Vec<_>>();
    assert!(locations_are_all_valid(bus_locations, &location_vector));

    let passenger_iter = passengers.iter();

    let passenger_locations = passenger_iter.flat_map(|passenger| {
        [
            (passenger.current_location).unwrap(),
            passenger.destination_location,
        ]
    });

    assert!(locations_are_all_valid(
        passenger_locations,
        &location_vector
    ));

    // Return the `Data`.
    Ok(data)
}

fn write_data_to_file(data: InputDataStructure, path: &Path) -> Result<(), Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::create(path)?;
    let writer = BufWriter::new(file);

    // Read the JSON contents of the file as an instance of `Data`.
    serde_json::to_writer_pretty(writer, &data)?;

    // Return the `Data`.
    Ok(())
}
const GLOBAL_PASSENGER_COUNT: usize = 10;
const GLOBAL_LOCATION_COUNT: usize = 5;
const BUS_CAPACITY: usize = 10;
const NUM_OF_BUSES: usize = 3;
const NUM_STOPS_PER_BUS: usize = 3;
const MAX_LOCATION_DISTANCE: u32 = 1;
const READ_JSON: bool = option_env!("READ_DATA").is_some();
const WRITE_JSON: bool = option_env!("WRITE_DATA").is_some();

fn main() {
    let initial_data: InputDataStructure = if READ_JSON {
        read_data_from_file(Path::new("bus_route_data.json")).unwrap()
    } else {
        let bus_routes = std::array::from_fn(|_| Vec::new());
        // bus_routes is only used if READ_JSON
        InputDataStructure {
            bus_routes,
            passengers: Vec::new(),
            location_vector: Vec::new(),
        }
    };

    let InputDataStructure {
        bus_routes,
        passengers,
        location_vector,
    } = initial_data;

    // How do I retrieve locations from the already produced list so that the locations are the same?
    let location_vector = if READ_JSON {
        location_vector
    } else {
        initialize_location_list(GLOBAL_LOCATION_COUNT)
    };

    let total_passenger_list = if READ_JSON {
        passengers
            .into_iter()
            .map(|passenger| passenger.into())
            .collect()
    } else {
        generate_random_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap()
    };

    let mut bus_route_array: [Vec<BusLocation>; NUM_OF_BUSES] = std::array::from_fn(|_| Vec::new());
    if READ_JSON {
        bus_route_array = bus_routes;
    } else {
        for bus_route in bus_route_array.iter_mut() {
            let next_bus_route = generate_bus_route_locations_with_distances(
                &location_vector,
                NUM_STOPS_PER_BUS,
                MAX_LOCATION_DISTANCE,
            );

            *bus_route = next_bus_route.unwrap();
        }
    }

    let rejected_passengers_pointer = Arc::new(Mutex::new(Vec::new()));

    let passenger_list_pointer = Arc::new(Mutex::new(total_passenger_list));

    let location_vector_arc = Arc::new(location_vector);

    let bus_route_vec_arc: Arc<Mutex<[Vec<BusLocation>; NUM_OF_BUSES]>> =
        Arc::new(Mutex::new(bus_route_array));

    let passenger_bus_route_arc: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let passenger_extra_stops_waited_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    // Split time ticks into two - time ticks are accurate
    let current_time_tick = Arc::new(Mutex::new(0));

    let program_end = Arc::new(Mutex::new(false));

    let mut handle_list = vec![];

    let sync_handle_program_end_clone = program_end.clone();

    let (tx_from_bus_threads, rx_from_threads) = mpsc::channel();

    let (tx_to_passengers, rx_to_passengers) = mpsc::channel();

    let (tx_stations_to_passengers, rx_stations_to_passengers) = mpsc::channel();
    let (send_to_station_channels, receive_in_station_channels) =
        initialize_channel_list(GLOBAL_LOCATION_COUNT);

    let send_to_station_channels_arc = Arc::new(send_to_station_channels);
    let receive_in_station_channels_arc = Arc::new(Mutex::new(receive_in_station_channels));

    let (send_to_bus_channels, receive_in_bus_channels) = initialize_channel_list(NUM_OF_BUSES);

    let bus_receiver_channels_arc = Arc::new(Mutex::new(receive_in_bus_channels));
    let send_to_bus_channels_arc = Arc::new(send_to_bus_channels);

    let current_time_tick_clone = current_time_tick.clone();

    let route_sync_bus_route_vec_arc = bus_route_vec_arc.clone();
    let route_sync_passenger_list_arc = passenger_list_pointer.clone();
    let route_sync_location_vec_arc = location_vector_arc.clone();

    let route_sync_handle = thread::spawn(move || {
        let passenger_sender = tx_to_passengers;
        let mut bus_status_array = [BusThreadStatus::Uninitialized; NUM_OF_BUSES];
        // Bus that has unloaded passengers/ moving buses
        let mut processed_bus_received_count = 0;
        let mut rejected_passengers_list = Vec::new();
        let mut processed_moving_bus_count = 0;

        // Eventually, there will be two time ticks per movement so that the stopped buses can have two ticks:
        // One for unloading passengers and another for loading them. Bus Movement will probably happen on the second tick
        // (Uninitialized items could contain the empty vector)

        // Buses are initialized on the first time tick, Passengers choose their bus route info on the second time tick, and the bus route happens on the third time tick

        loop {
            println!("Route sync loop beginning");
            println!("processed bus received count: {processed_bus_received_count}");
            if processed_bus_received_count
                == bus_status_array
                    .iter()
                    .filter(|status| *status != &BusThreadStatus::BusFinishedRoute)
                    .count()
            {
                processed_bus_received_count = 0;
                println!("Moving bus processed count: {}", processed_moving_bus_count);
                processed_moving_bus_count = 0;
                if rejected_passengers_list.is_empty() {
                    println!("No Rejected Passengers to send to the other thread");
                    passenger_sender.send(None).unwrap();

                    continue;
                }

                println!(
                    "There were rejected passengers received. Count: {}",
                    rejected_passengers_list.len()
                );

                println!("Rejected Passenger List: {:#?}", rejected_passengers_list);
                println!("List Length: {}", rejected_passengers_list.len());
                println!("Rejected passengers for all buses were received");
                passenger_sender
                    .send(Some(rejected_passengers_list.clone()))
                    .unwrap();

                rejected_passengers_list.clear();
            }
            println!("Before receiving a message.");
            let received_bus_stop_message = rx_from_threads.recv().unwrap();

            println!("Message received: {:?}", received_bus_stop_message);
            let mut current_time_tick = current_time_tick_clone.lock().unwrap();
            println!("Message Received. Time tick: {}", current_time_tick);
            println!(
                "Before time processing, bus processed count: {}",
                processed_bus_received_count
            );
            match received_bus_stop_message {
                BusMessages::AdvanceTimeStep {
                    // current_time_step,
                    bus_index,
                    ..
                } => {
                    // println!(
                    //     "Time step recieved from Bus {}. Time tick {}",
                    //     bus_number, &current_time_tick
                    // );
                    bus_status_array[bus_index] = BusThreadStatus::CompletedTimeStep;
                }

                BusMessages::BusFinished { bus_index } => {
                    bus_status_array[bus_index] = BusThreadStatus::BusFinishedRoute;
                    println!("Bus {} Finished Route", bus_index);
                    let finished_buses = bus_status_array
                        .iter()
                        .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                        .count();
                    let active_buses = NUM_OF_BUSES - finished_buses;
                    println!("There are now {active_buses} active buses.");
                }

                BusMessages::InitBus { bus_index } => {
                    bus_status_array[bus_index] = BusThreadStatus::WaitingForTimeStep;
                    println!("Bus {bus_index} Initialized");
                    // println!("Bus Route list: {:#?}", *bus_route_clone.lock().unwrap());
                }
                BusMessages::InitPassengers => {
                    *current_time_tick += 1;
                    println!("Passenger initialized");
                }

                BusMessages::RejectedPassengers(RejectedPassengersMessages::MovingBus) => {
                    println!("Moving bus received");
                    processed_bus_received_count += 1;
                    processed_moving_bus_count += 1;
                }

                BusMessages::RejectedPassengers(RejectedPassengersMessages::StoppedBus {
                    ref rejected_passengers,
                }) => {
                    println!("Stopped Bus Received");
                    rejected_passengers_list.append(&mut rejected_passengers.clone());
                    processed_bus_received_count += 1;
                }

                BusMessages::RejectedPassengers(
                    RejectedPassengersMessages::CompletedProcessing,
                ) => {
                    println!("Rejected Passengers were all processed");
                    *current_time_tick += 1;
                }
            }
            println!("Processed received: {processed_bus_received_count}");
            println!(
                "{}",
                bus_status_array
                    .iter()
                    .filter(|status| *status != &BusThreadStatus::BusFinishedRoute)
                    .count()
            );
            // There might be a way to
            println!("Rejected Passengers conditional");
            if let BusMessages::RejectedPassengers(_) = received_bus_stop_message {}

            if *current_time_tick == 0
                && bus_status_array
                    .iter()
                    .filter(|status| *status == &BusThreadStatus::Uninitialized)
                    .count()
                    == 0
            {
                *current_time_tick += 1;
                if WRITE_JSON {
                    let location_vector = route_sync_location_vec_arc.as_ref();
                    let passenger_list: Vec<SerializedPassenger> = route_sync_passenger_list_arc
                        .lock()
                        .unwrap()
                        .clone()
                        .iter()
                        .map(|passenger| passenger.clone().into())
                        .collect();
                    let bus_route_list = route_sync_bus_route_vec_arc.lock().unwrap().clone();
                    let json_structure = InputDataStructure {
                        bus_routes: bus_route_list,
                        passengers: passenger_list,
                        location_vector: location_vector.clone(),
                    };
                    write_data_to_file(json_structure, Path::new("bus_route_data.json")).unwrap();
                }
                println!(
                    "All Buses Initialized. Time tick 0 message: {:?}",
                    received_bus_stop_message
                );
                drop(current_time_tick);
                continue;
            }

            let finished_buses = bus_status_array
                .iter()
                .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                .count();
            println!("There are {finished_buses} finished buses.");

            if finished_buses >= NUM_OF_BUSES {
                let mut program_end = sync_handle_program_end_clone.lock().unwrap();
                *program_end = true;
                println!("Program Complete");
                break;
            }

            // Increment the time step after all buses have run
            if bus_status_array
                .iter()
                .filter(|status| *status == &BusThreadStatus::WaitingForTimeStep)
                .count()
                == 0
            {
                for status in bus_status_array.iter_mut() {
                    if status == &BusThreadStatus::CompletedTimeStep {
                        *status = BusThreadStatus::WaitingForTimeStep;
                    }
                }
                println!("---------- All buses are finished at their stops -----------");
                *current_time_tick += 1;
                // buses_finished_at_stops = 0;
            }
            println!("End of sync loop");
        }
    });

    handle_list.push(route_sync_handle);

    // Valid route list of top 3 solutions
    // A* algorithm eventually
    // When a passenger is rejected on a certain bus,
    // trip cost should be less than some value

    // PassengerOnboardingBusSchedule => PassengerOnboardingTimeTick, PassengerLeavingTimeTick

    // return Option with a tuple containing bus number and pick up time tick

    let passenger_thread_bus_route_clone = bus_route_vec_arc.clone();

    let passenger_thread_passenger_list_clone = passenger_list_pointer.clone();

    let passenger_thread_program_end_clone = program_end.clone();

    let passenger_thread_passenger_bus_route_clone = passenger_bus_route_arc.clone();

    let passenger_thread_time_tick_clone = current_time_tick.clone();
    let passenger_thread_sender = tx_from_bus_threads.clone();
    let station_sender_list = send_to_station_channels_arc.clone();
    let passengers_thread_handle = thread::spawn(move || {
        let stations_receiver = rx_stations_to_passengers;
        let mut rejected_passengers: Vec<Passenger> = Vec::new();
        let receiver_from_sync_thread = rx_to_passengers;
        let mut previous_time_tick = 0;
        loop {
            // println!("Passenger loop start");
            // Wait for 1 milliseconds to give other threads a chance to use the time tick mutex
            // std::thread::sleep(std::time::Duration::from_millis(1));
            // let mut rejected_passengers_indeces: Vec<usize> = Vec::new();
            let time_tick = passenger_thread_time_tick_clone.lock().unwrap();
            // println!("Passenger loop beginning. Time tick: {}", time_tick);
            // println!("Passenger thread start.");

            if *passenger_thread_program_end_clone.lock().unwrap() {
                println!("Rejected Passenger count: {}", rejected_passengers.len());
                break;
            }

            if *time_tick == 0 || *time_tick % 2 == 0 || previous_time_tick == *time_tick {
                drop(time_tick);
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            } else {
                previous_time_tick = *time_tick;
            }
            let bus_route_list = passenger_thread_bus_route_clone.lock().unwrap();
            let mut passenger_bus_route_list =
                passenger_thread_passenger_bus_route_clone.lock().unwrap();

            *passenger_bus_route_list = bus_route_list
                .clone()
                .into_iter()
                .map(convert_bus_route_list_to_passenger_bus_route_list)
                .collect();

            drop(passenger_bus_route_list);

            println!("Bus route list: {bus_route_list:#?}");
            drop(bus_route_list);

            // It may not be neccessary to do this on the first time tick
            if *time_tick == 1 {
                drop(time_tick);
                println!("First time tick loop");
                let passenger_list = passenger_thread_passenger_list_clone.lock().unwrap();
                println!("Beginning of tick one passenger calculations");
                /* for (passenger_index, passenger) in passenger_list.iter_mut().enumerate() {
                    match passenger.status {
                        PassengerStatus::Waiting => {
                            // If passenger is waiting for the bus, find out what bus will be able to take
                            // the passenger to his destination at a time later than this time step
                            // and send some message to the bus to pick him up
                            // println!("Passenger Loop started on tick 1");

                            if let Ok(bus_schedule) = find_bus_to_pick_up_passenger(
                                passenger,
                                *time_tick,
                                bus_route_list.clone(),
                            ) {
                                println!("Passenger Schedule: {bus_schedule:?}");
                                passenger.bus_schedule = bus_schedule.clone();
                            } else {
                                println!("Some passengers were rejected");
                                rejected_passengers_indeces.push(passenger_index);
                            }
                            // println!("Passenger Loop ended on tick 1");
                        }
                        PassengerStatus::OnBus => {
                            // If the passenger is on a bus, perhaps send a message to get off of bus
                            // if the bus has arrived? That could also be done by the bus.
                        }
                        PassengerStatus::Arrived => {
                            // If passenger is at destination, there is nothing to be done,
                            // since the passenger has arrived and is not part of the bus route anymore
                        }
                    }
                } */

                let mut passenger_location_list: Vec<Vec<Passenger>> = Vec::new();
                for _ in 0..GLOBAL_LOCATION_COUNT {
                    passenger_location_list.push(Vec::new());
                }
                /* for (passenger_index, passenger) in passenger_list.iter().enumerate() {
                    let current_location_index = passenger.current_location.unwrap().index;
                    passenger_location_list[current_location_index].push(passenger.clone())
                } */

                let mut location_vec_dequeue = VecDeque::from(passenger_list.clone());

                while let Some(passenger) = location_vec_dequeue.pop_back() {
                    let current_location_index = passenger.current_location.unwrap().index;
                    passenger_location_list[current_location_index].push(passenger);
                }

                for (index, passengers_in_location) in
                    passenger_location_list.into_iter().enumerate()
                {
                    station_sender_list.as_ref()[index]
                        .send(StationMessages::InitPassengerList(passengers_in_location))
                        .unwrap();
                }

                for _ in 0..GLOBAL_LOCATION_COUNT {
                    let sync_message = stations_receiver.recv().unwrap();
                    if let StationToPassengersMessages::ConfirmInitPassengerList(station_number) =
                        sync_message
                    {
                        println!("Passenger Init Confirmed from Station {station_number}");
                    }
                }

                println!("End of tick one passenger calculations");

                // Remove passengers who cannot get onto a bus, since if they cannot get on any bus
                // now, they will not be able to later, because the schedules will not change. I
                // might as well continue keeping track of them.

                /* for passenger_index in rejected_passengers_indeces.into_iter().rev() {
                    let rejected_passenger = passenger_list.remove(passenger_index);
                    println!("Rejected passenger removed");
                    rejected_passengers.push(rejected_passenger);
                } */

                passenger_thread_sender
                    .send(BusMessages::InitPassengers)
                    .unwrap();
                println!("Passengers init message sent");
                // break;
                assert_eq!(
                    passenger_list.len() + rejected_passengers.len(),
                    GLOBAL_PASSENGER_COUNT as usize
                );

                break;
            } /*else {
                  // Otherwise, the time tick is odd

                  let mut passenger_list = passenger_thread_passenger_list_clone.lock().unwrap();
                  println!("Passenger rejected thread time tick: {}", time_tick);

                  // drop time_tick so that the lock is released before waiting for a message
                  drop(time_tick);

                  let rejected_passenger_list_option = receiver_from_sync_thread.recv().unwrap();
                  println!("Rejected Passenger Option: {rejected_passenger_list_option:#?}");
                  let time_tick = passenger_thread_time_tick_clone.lock().unwrap();

                  println!("Processed Bus Process Finished Message Received");
                  if let Some(mut rejected_passenger_list) = rejected_passenger_list_option {
                      // Somehow, the message only prints out once, yet around 490 passengers were rejected. Something is probably off.
                      println!(
                          "Some passengers were rejected. Count: {}",
                          rejected_passenger_list.len()
                      );
                      let mut nonboardable_passengers_list = vec![];
                      let mut nonboardable_passenger_indeces = vec![];
                      println!("Passenger loop started");
                      for mut passenger in rejected_passenger_list.into_iter() {
                          match passenger.status {
                              PassengerStatus::Waiting => {
                                  // If passenger is waiting for the bus, find out what bus will be able to take
                                  // the passenger to his destination at a time later than this time step
                                  // and send some message to the bus to pick him up

                                  if let Ok(bus_schedule) = calculate_passenger_bus_schedule(
                                      passenger.clone(),
                                      *time_tick,
                                      bus_route_list.clone(),
                                  ) {
                                      passenger.bus_schedule = bus_schedule.clone();
                                      println!("Accepted passenger: {:#?}", passenger);
                                      passenger_list
                                          .iter_mut()
                                          .filter(|list_passenger| {
                                              list_passenger.id == passenger.clone().id
                                          })
                                          .for_each(|filtered_passenger| {
                                              filtered_passenger.bus_schedule = bus_schedule.clone()
                                          });
                                  } else {
                                      nonboardable_passengers_list.push(passenger);
                                  }
                              }
                              PassengerStatus::OnBus => {
                                  // If the passenger is on a bus, perhaps send a message to get off of bus
                                  // if the bus has arrived? That could also be done by the bus.
                              }
                              PassengerStatus::Arrived => {
                                  // If passenger is at destination, there is nothing to be done,
                                  // since the passenger has arrived and is not part of the bus route anymore
                              }
                          }
                      }
                      println!("Passenger Loop ended");

                      // take care of unsuccessfully recalculated passengers
                      for nonboardable_passenger in nonboardable_passengers_list {
                          for (passenger_index, passenger) in
                              passenger_list.clone().into_iter().enumerate()
                          {
                              if nonboardable_passenger == passenger {
                                  nonboardable_passenger_indeces.push(passenger_index);
                                  break;
                              }
                          }
                      }

                      nonboardable_passenger_indeces.sort();

                      println!("Passenger List: {:#?}", passenger_list);

                      // Could be duplicate passengers

                      for passenger_index in nonboardable_passenger_indeces.into_iter().rev() {
                          println!("Rejected passenger removed in later stage");
                          let rejected_passenger = passenger_list.remove(passenger_index);
                          rejected_passengers.push(rejected_passenger);
                      }

                      println!("Non active passengers count: {}", rejected_passengers.len());

                      let finished_passenger_count = passenger_list
                          .iter()
                          .filter(|passenger| passenger.status == PassengerStatus::Arrived)
                          .count();

                      // This print statement seems useless becaure finished_passenger_count is always 0, for some reason
                      println!(
                      "{finished_passenger_count} passengers successfully arived at their destination"
                      );
                      println!(
                          "Passenger status list: {:#?}",
                          passenger_list
                              .iter()
                              .map(|passenger| passenger.status)
                              .collect::<Vec<_>>()
                      );

                      assert_eq!(
                          passenger_list.len() + rejected_passengers.len(),
                          GLOBAL_PASSENGER_COUNT as usize
                      );

                      // Remove passengers who cannot get onto a bus, since if they cannot get on any bus
                      // now, they will not be able to later, because the schedules will not change. I
                      // might as well continue keeping track of them.

                      // for passenger_index in rejected_passengers_indeces.into_iter().rev() {
                      //     let rejected_passenger = rejected_passenger_list.remove(passenger_index);
                      //     rejected_passengers.push(rejected_passenger);
                      // }
                  }

                  passenger_thread_sender
                      .send(BusMessages::RejectedPassengers(
                          RejectedPassengersMessages::CompletedProcessing,
                      ))
                      .unwrap();

                  previous_time_tick = *time_tick;

                  drop(time_tick);
              } */
        }

        let total_rejected_passengers = rejected_passengers.len();
        println!("There were a total of {total_rejected_passengers} rejected passengers");
    });

    handle_list.push(passengers_thread_handle);

    let station_location_list = location_vector_arc.clone();

    // Station index and location index are equivilant
    for (location_index, location) in station_location_list.iter().enumerate() {
        let station_index = location_index;
        let station_time_tick = current_time_tick.clone();
        let current_location = *location;
        let send_to_bus_channels = send_to_bus_channels_arc.clone();
        let station_channels = receive_in_station_channels_arc.clone();
        let bus_route_list = bus_route_vec_arc.clone();
        let station_thread_passenger_bus_route_list = passenger_bus_route_arc.clone();
        // let send_to_bus_channels =
        let rejected_passenger_clone = rejected_passengers_pointer.clone();

        let to_passengers_sender_clone = tx_stations_to_passengers.clone();
        let station_handle = thread::spawn(move || {
            let mut current_station = Station::new(current_location);
            let mut previous_time_tick = 0;
            let current_receiver = station_channels.lock().unwrap().remove(0);

            loop {
                // println!("Station {} loop beginning", station_index);
                let time_tick = station_time_tick.lock().unwrap();
                // println!(
                //     "Station {} loop beginning. Time tick: {}",
                //     station_index, time_tick
                // );
                // println!("Station {location_index}. time tick: {}", *time_tick);

                // When time tick is 0, previous_time_tick is 0, so the loop is skipped. This is fine, since the station
                // does not do anything on time tick 0.
                if previous_time_tick != *time_tick {
                    previous_time_tick = *time_tick;
                } else {
                    drop(time_tick);
                    continue;
                }

                if *time_tick == 1 {
                    println!("First time tick station {} thread", station_index);
                    drop(time_tick);
                    let received_message = current_receiver.recv().unwrap();
                    let time_tick = station_time_tick.lock().unwrap();
                    if let StationMessages::InitPassengerList(mut list) = received_message {
                        println!("Station {station_index} Message: {list:#?}");
                        let mut rejected_passenger_indeces = Vec::new();
                        for (index, passenger) in list.iter().enumerate() {
                            // let passenger_bus_route_list =
                            //     station_thread_passenger_bus_route_list.lock().unwrap();
                            println!("Passenger will be added to station {}", station_index);
                            current_station
                                .add_passenger(
                                    passenger.clone(),
                                    *time_tick,
                                    &station_thread_passenger_bus_route_list.lock().unwrap(),
                                )
                                .unwrap_or_else(|passenger| {
                                    rejected_passenger_indeces.push(index);
                                    println!("Passenger {:?} had no valid routes", passenger)
                                });
                        }

                        for passenger_index in rejected_passenger_indeces.into_iter().rev() {
                            let removed_passenger = list.remove(passenger_index);
                            (*rejected_passenger_clone.lock().unwrap()).push(removed_passenger);
                        }
                        println!("Passengers added to station {}", station_index);
                        drop(time_tick);
                        println!("total passenger_count: {}", list.len());
                        to_passengers_sender_clone
                            .send(StationToPassengersMessages::ConfirmInitPassengerList(
                                station_index,
                            ))
                            .unwrap();
                    } else {
                        panic!("{received_message:?} should not be sent at time tick 1.");
                    }
                    continue;
                }

                drop(time_tick);

                let received_message = current_receiver.recv().unwrap();
                let time_tick = station_time_tick.lock().unwrap();
                match received_message {
                    StationMessages::InitPassengerList(message) => panic!(
                        "PassengerInit message should not be sent on any time tick besides tick 1. Time tick: {}, List sent: {:#?}", time_tick, message 
                    ),
                    StationMessages::BusArrived(bus) => {
                        let bus_index = bus.bus_num;
                        println!("Bus {bus_index} arrived at station {station_index}.");
                        current_station.docked_buses.push(bus);
                        (send_to_bus_channels.as_ref())[bus_index]
                            .send(StationToBusMessages::AcknowledgeArrival())
                            .unwrap();
                    }
                }
            }
        });

        handle_list.push(station_handle);
    }

    for bus_index in 0..NUM_OF_BUSES {
        let bus_route_array_clone = bus_route_vec_arc.clone();
        let station_senders_clone = send_to_station_channels_arc.clone();
        let sender = tx_from_bus_threads.clone();
        let bus_receiver_channels = bus_receiver_channels_arc.clone();

        // let passenger_list_pointer_clone = passenger_list_pointer.clone();
        // let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let current_time_tick_clone = current_time_tick.clone();
        let mut time_clone_check = 1;
        let handle = thread::spawn(move || {
            let mut bus_route_array = bus_route_array_clone.lock().unwrap();
            let bus_route = bus_route_array.get(bus_index).unwrap();
            println!("Bus {bus_index} bus route: {bus_route:#?}");
            let current_bus_receiver = bus_receiver_channels.lock().unwrap().remove(0);
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY, bus_index);
            println!("Bus {bus_index} created");
            bus_route_array[simulated_bus.bus_num] = simulated_bus.get_bus_route();
            // Release the lock on bus_route_array by dropping it
            drop(bus_route_array);
            sender
                .send(BusMessages::InitBus {
                    bus_index: simulated_bus.bus_num,
                })
                .unwrap();
            println!("Bus message sent");

            /* loop {
                // println!("Bus loop beginning");
                let current_time_tick = current_time_tick_clone.lock().unwrap();
                // println!("Bus thread start");
                if *current_time_tick < 2 || *current_time_tick % 2 == 1 {
                    continue;
                }

                // Make sure the loop only runs once for each time tick
                if time_clone_check == *current_time_tick {
                    continue;
                } else {
                    time_clone_check = *current_time_tick;
                }

                println!("Current time tick: {}", &current_time_tick);

                // Why is this message sent exactly twice for each time tick?
                let update_option = simulated_bus.update(
                    &mut passenger_list_pointer_clone.lock().unwrap(),
                    &mut passenger_stops_passed_pointer_clone.lock().unwrap(),
                    &sender,
                    &current_time_tick,
                );

                drop(current_time_tick);

                match update_option {
                    ControlFlow::Break(()) => break,
                    // ControlFlow::Continue(UpdateOutput::WrongTimeTick) => {}
                    ControlFlow::Continue(UpdateOutput::MovingBus) => sender
                        .send(BusMessages::RejectedPassengers(
                            RejectedPassengersMessages::MovingBus,
                        ))
                        .unwrap(),
                    ControlFlow::Continue(UpdateOutput::ReceivedPassengers {
                        rejected_passengers,
                    }) => sender
                        .send(BusMessages::RejectedPassengers(
                            RejectedPassengersMessages::StoppedBus {
                                rejected_passengers,
                            },
                        ))
                        .unwrap(),
                }
                println!("Bus loop end.");
            }

            sender
                .send(BusMessages::BusFinished {
                    bus_index: simulated_bus.bus_num,
                })
                .unwrap(); */
            loop {
                // println!("Bus {} Loop Beginning", simulated_bus.bus_num);
                let current_time_tick = current_time_tick_clone.lock().unwrap();
                // println!("Bus loop beginning. time tick: {current_time_tick}");
                if *current_time_tick < 2 {
                    drop(current_time_tick);
                    continue;
                }
                // println!(
                //     // Note, only one bus ever prints this at a time
                //     "Bus {} bus route loop. Time tick: {}",
                //     { simulated_bus.bus_num },
                //     current_time_tick
                // );
                if time_clone_check == *current_time_tick {
                    // println!("Time tick skipped");
                    drop(current_time_tick);
                    continue;
                } else {
                    time_clone_check = *current_time_tick;
                }

                drop(current_time_tick);
                simulated_bus.update(station_senders_clone.as_ref(), &sender); //&current_time_tick);
            }
        });
        handle_list.push(handle);
    }
    for handle in handle_list {
        handle.join().unwrap();
    }

    let passenger_extra_stops_waited = passenger_extra_stops_waited_pointer.lock().unwrap();
    let total_extra_stops_waited: u32 = passenger_extra_stops_waited.iter().sum();

    let total_passengers = passenger_extra_stops_waited.len();

    // let passengers_remaining = passenger_list_pointer.lock().unwrap().len();
    let passengers_remaining = GLOBAL_PASSENGER_COUNT - total_passengers;

    println!("{passengers_remaining} passengers did not get onto any bus");

    println!("total extra stops waited: {}", total_extra_stops_waited);

    println!(
        "Average wait time between stops {}",
        total_extra_stops_waited as f64 / total_passengers as f64
    );
}
