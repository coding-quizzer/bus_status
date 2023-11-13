use std::{
    hash::Hash,
    ops::ControlFlow,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread::{self, current},
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

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
struct Location {
    id: Uuid,
}

impl Location {
    fn new() -> Location {
        Location { id: Uuid::new_v4() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PassengerStatus {
    OnBus,
    Waiting,
    Arrived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PassengerOnboardingBusSchedule {
    time_tick: u32,
    // the last destination will not include a bus number because the passenger will be at his destination
    bus_num: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, PartialEq, Eq, Debug)]
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

#[derive(Debug, Clone)]
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
    time_tick_num: u32,
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

// let passengers take the most efficient route

#[derive(Debug, Clone)]
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
    WrongTimeTick,
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
            time_tick_num: 0,
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

    // Why is the AdvanceTimeStep message sent twice for each bus?
    fn update(
        &mut self,
        waiting_passengers: &mut [Passenger],
        passenger_stops_waited_list: &mut Vec<u32>,
        sender: &Sender<BusMessages>, // bus_num: u32,
        current_time_tick_number: &u32,
    ) -> ControlFlow<(), UpdateOutput> {
        println!("Update Beginning");
        println!(
            "Bus update. current time tick: {}",
            current_time_tick_number
        );
        if self.time_tick_num < *current_time_tick_number {
            // This might cause problems, since it sends a message for increaseing the time step
            // before the operations have actually been performed. Hopefully keeping the timestep
            // locked until after this function shoudl help?
            println!(
                "Bus Time tick sent. Time tick: {}",
                current_time_tick_number
            );

            // Bus message
            // Holding output in a variable can allow a single send directly before the return
            // sender should be right before the

            println!("Bus Number {} Sent", self.bus_num);
            if let MovementState::Moving(distance) = self.status.movement {
                if distance > 0 {
                    println!("Bus {} distance to next stop: {}", self.bus_num, distance);
                    self.status.movement = MovementState::Moving(distance - 1);
                    // return Some(());
                } else {
                    println!("Bus {}, will stop at next time tick", self.bus_num);
                    self.stop_at_destination_stop();
                }
                self.time_tick_num += 1;
                sender
                    .send(BusMessages::AdvanceTimeStep {
                        //current_time_step: self.time_tick_num,
                        bus_number: self.bus_num,
                    })
                    .unwrap_or_else(|error| panic!("Error from bus {}: {}", self.bus_num, error));
                return ControlFlow::Continue(UpdateOutput::MovingBus);
            } else {
                println!("Bus {} stopped", self.bus_num);
                let more_locations_left = self.leave_for_next_location();

                self.drop_off_passengers(passenger_stops_waited_list);
                let rejected_passengers =
                    self.take_passengers(waiting_passengers, current_time_tick_number);

                self.time_tick_num += 1;

                if more_locations_left.is_some() {
                    sender
                        .send(BusMessages::AdvanceTimeStep {
                            //current_time_step: self.time_tick_num,
                            bus_number: self.bus_num,
                        })
                        .unwrap_or_else(|error| {
                            panic!("Error from bus {}: {}", self.bus_num, error)
                        });
                    return ControlFlow::Continue(UpdateOutput::ReceivedPassengers {
                        rejected_passengers,
                    });
                };

                assert_eq!(self.passengers.len(), 0);
                println!("Bus number {} is finished", self.bus_num);
                self.status.movement = MovementState::Finished;
                return ControlFlow::Break(());
            }
        }
        // ControlFlow::Continue(None) Means either this is the wrong timestep, or else the bus is moving
        ControlFlow::Continue(UpdateOutput::WrongTimeTick)
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
                break;
            }

            let PassengerOnboardingBusSchedule {
                time_tick: onboarding_time_tick,
                bus_num,
            } = passenger
                .bus_schedule
                .get(0)
                .expect("Passenger schedule cannot be empty");

            if onboarding_time_tick == current_time_tick && bus_num.expect("At this point, this cannot be the last bus loction, and thus the bus_num must exist") == self.bus_num {
              println!("This is the correct time tick and bus");
              if self.passengers.len() >= self.capacity {
                  println!("Passenger Rejected. Bus Overfull");
                  overflow_passengers.push(passenger.clone());
                  break;
              }
                let onboard_passenger = passenger.convert_to_onboarded_passenger();
                self.add_passenger(onboard_passenger);
            }

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
        overflow_passengers
    }
    fn drop_off_passengers(&mut self, passenger_passed_stops: &mut Vec<u32>) -> Option<()> {
        println!("Drop off Passengers");
        let current_location = self.current_location?;
        let bus_passengers = &mut *self.passengers;
        let mut new_bus_passengers = vec![];
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

#[derive(PartialEq, Debug)]
enum RejectedPassengersMessages {
    MovingBus,
    StoppedBus { rejected_passengers: Vec<Passenger> },
    CompletedProcessing,
}

#[derive(PartialEq, Debug)]
enum BusMessages {
    InitBus {
        bus_number: usize,
    },
    InitPassengers,
    AdvanceTimeStep {
        // current_time_step: u32,
        bus_number: usize,
    },
    BusFinished {
        bus_number: usize,
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

fn generate_passenger(location_list: &Vec<Location>) -> Result<Passenger, String> {
    let location_vector = generate_list_of_random_elements_from_list(location_list, 2)?;

    let [old_location, new_location] = location_vector[..] else {
        panic!("Returned Vector was invalid")
    };

    Ok(Passenger::new(old_location, new_location))
}

fn generate_passenger_list(
    count: u32,
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
    let mut time_tick = 0;
    let mut passenger_bus_route_list = vec![];
    for (index, bus_location) in bus_route_list.iter().enumerate() {
        let mut index_increment = 0;
        if index != 0 {
            // Add one to the index_increment for the time tick used at the previous stop
            index_increment += 1;
        }

        index_increment += bus_location.distance_to_location;

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
) -> Result<Vec<BusLocation>, String> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bus_route_list = generate_bus_route_locations(location_list, length);
    let bus_route_list_to_bus_location_types = bus_route_list?
        .iter()
        .map(|location| BusLocation {
            location: *location,
            distance_to_location: rng.gen_range(1..=5),
        })
        .collect::<Vec<_>>();
    Ok(bus_route_list_to_bus_location_types)
}

fn initialize_location_list(count: u32) -> Vec<Location> {
    let mut location_list = vec![];
    for _ in 0..count {
        location_list.push(Location::new());
    }
    location_list
}

const GLOBAL_PASSENGER_COUNT: u32 = 500;
const GLOBAL_LOCATION_COUNT: u32 = 10;
const BUS_CAPACITY: usize = 10;
const NUM_OF_BUSES: usize = 4;
const NUM_STOPS_PER_BUS: usize = 25;

fn main() {
    let location_vector = initialize_location_list(GLOBAL_LOCATION_COUNT);

    let total_passenger_list =
        generate_passenger_list(GLOBAL_PASSENGER_COUNT, &location_vector).unwrap();

    let passenger_list_pointer = Arc::new(Mutex::new(total_passenger_list));

    let location_vector_arc = Arc::new(location_vector);

    let bus_route_vec_arc: Arc<Mutex<[Vec<BusLocation>; NUM_OF_BUSES]>> =
        Arc::new(Mutex::new(std::array::from_fn(|_| vec![])));

    let passenger_extra_stops_waited_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    // Split time ticks into two - time ticks are accurate
    let current_time_tick = Arc::new(Mutex::new(0));

    let program_end = Arc::new(Mutex::new(false));

    let mut handle_list = vec![];

    let sync_handle_program_end_clone = program_end.clone();

    let (tx_from_threads, rx_from_threads) = mpsc::channel();

    let (tx_to_passengers, rx_to_passengers) = mpsc::channel();

    let current_time_tick_clone = current_time_tick.clone();

    let route_sync_handle = thread::spawn(move || {
        let passenger_sender = tx_to_passengers;
        let mut bus_status_array = [BusThreadStatus::Uninitialized; NUM_OF_BUSES];
        // Bus that has unloaded passengers/ moving buses
        let mut rejected_bus_received_count = 0;
        let mut rejected_passengers_list = Vec::new();
        let mut processed_moving_bus_count = 0;

        // Eventually, there will be two time ticks per movement so that the stopped buses can have two ticks:
        // One for unloading passengers and another for loading them. Bus Movement will probably happen on the second tick
        // (Uninitialized items could contain the empty vector)

        // Buses are initialized on the first time tick, Passengers choose their bus route info on the second time tick, and the bus route happens on the third time tick

        loop {
            println!("rejected bus received count: {rejected_bus_received_count}");
            if rejected_bus_received_count
                == bus_status_array
                    .iter()
                    .filter(|status| *status != &BusThreadStatus::BusFinishedRoute)
                    .count()
            {
                rejected_bus_received_count = 0;
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
            let received_bus_stop_message = rx_from_threads.recv().unwrap();
            println!("Sync thread loop");
            let mut current_time_tick = current_time_tick_clone.lock().unwrap();
            println!("Message Received. Time tick: {}", current_time_tick);
            println!("Message: {:?}", received_bus_stop_message);
            println!(
                "Before time processing, bus processed count: {}",
                rejected_bus_received_count
            );
            match received_bus_stop_message {
                BusMessages::AdvanceTimeStep {
                    // current_time_step,
                    bus_number,
                    ..
                } => {
                    // println!(
                    //     "Time step recieved from Bus {}. Time tick {}",
                    //     bus_number, &current_time_tick
                    // );
                    bus_status_array[bus_number - 1] = BusThreadStatus::CompletedTimeStep;
                }

                BusMessages::BusFinished { bus_number } => {
                    bus_status_array[bus_number - 1] = BusThreadStatus::BusFinishedRoute;
                    println!("Bus {} Finished Route", bus_number);
                    let finished_buses = bus_status_array
                        .iter()
                        .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                        .count();
                    let active_buses = NUM_OF_BUSES - finished_buses;
                    println!("There are now {active_buses} active buses.");
                }

                BusMessages::InitBus { bus_number } => {
                    bus_status_array[bus_number - 1] = BusThreadStatus::WaitingForTimeStep;
                    println!("Bus {bus_number} Initialized")
                }
                BusMessages::InitPassengers => {
                    *current_time_tick += 1;
                    println!("Passenger initialized");
                }

                BusMessages::RejectedPassengers(RejectedPassengersMessages::MovingBus) => {
                    println!("Moving bus received");
                    rejected_bus_received_count += 1;
                    processed_moving_bus_count += 1;
                }

                BusMessages::RejectedPassengers(RejectedPassengersMessages::StoppedBus {
                    ref rejected_passengers,
                }) => {
                    println!("Stopped Bus Received");
                    rejected_passengers_list.append(&mut rejected_passengers.clone());
                    rejected_bus_received_count += 1;
                }

                BusMessages::RejectedPassengers(
                    RejectedPassengersMessages::CompletedProcessing,
                ) => {
                    println!("Rejected Passengers were all processed");
                    *current_time_tick += 1;
                }
            }
            println!("Rejected buses received: {rejected_bus_received_count}");
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
                println!(
                    "All Buses Initialized. Time tick 0 message: {:?}",
                    received_bus_stop_message
                );
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
        }
    });

    handle_list.push(route_sync_handle);

    // Valid route list of top 3 solutions
    // A* algorithm eventually
    // When a passenger is rejected on a certain bus,
    // trip cost should be less than some value

    // PassengerOnboardingBusSchedule => PassengerOnboardingTimeTick, PassengerLeavingTimeTick

    // return Option with a tuple containing bus number and pick up time tick
    fn find_bus_to_pick_up_passenger(
        passenger: &Passenger,
        time_tick: u32,
        bus_route_list: Vec<Vec<PassengerBusLocation>>,
    ) -> Result<Vec<PassengerOnboardingBusSchedule>, &Passenger> {
        // let start_plan: PassengerOnboardingBusSchedule;
        // let end_plan: PassengerOnboardingBusSchedule;
        let current_location = passenger.current_location;
        let destination_location = passenger.destination_location;
        let mut destination_route_hash: std::collections::HashMap<_, _> =
            std::collections::HashMap::new();
        for (bus_index, bus_route) in bus_route_list.iter().enumerate() {
            let hash_stored_route: Option<&Vec<PassengerOnboardingBusSchedule>> =
                destination_route_hash.get(&(current_location, destination_location));
            // let bus_route_iter = bus_route.iter();

            match hash_stored_route {
                Some(route) => return Ok(route.clone()),
                None => {
                    for passenger_bus_location in bus_route {
                        // let PassengerBusLocation {
                        //     location,
                        //     location_time_tick,
                        // } = passenger_bus_location;
                        if passenger_bus_location.location == current_location.unwrap()
                            && (passenger_bus_location.location_time_tick * 2 + 2) > time_tick
                        {
                            for other_passenger_bus_location in bus_route {
                                let PassengerBusLocation {
                                    location: location_for_dest,
                                    location_time_tick: time_tick_for_dest,
                                } = other_passenger_bus_location;

                                if other_passenger_bus_location.location == destination_location
                                    && other_passenger_bus_location.location_time_tick
                                        > passenger_bus_location.location_time_tick
                                {
                                    let destination_route = vec![
                                        PassengerOnboardingBusSchedule {
                                            bus_num: Some(bus_index + 1),
                                            // multiply by two to skip time when passengers are generated
                                            // and add two to skip the initial two time ticks
                                            // TODO: Adjust again when a seperat time tick is used for loading and unloading
                                            time_tick: (passenger_bus_location.location_time_tick)
                                                * 2
                                                + 2,
                                        },
                                        PassengerOnboardingBusSchedule {
                                            bus_num: None,
                                            // See above
                                            time_tick: (other_passenger_bus_location
                                                .location_time_tick)
                                                * 2
                                                + 2,
                                        },
                                    ];

                                    destination_route_hash.insert(
                                        (current_location, destination_location),
                                        destination_route.clone(),
                                    );
                                    return Ok(destination_route);
                                }
                            }
                        }
                    }
                }
            }
        }

        println!("Bus route loop found no routes");

        Err(passenger)
    }

    let passenger_thread_bus_route_clone = bus_route_vec_arc.clone();

    let passenger_thread_passenger_list_clone = passenger_list_pointer.clone();

    let passenger_thread_program_end_clone = program_end.clone();

    let passenger_thread_time_tick_clone = current_time_tick.clone();
    let passenger_thread_sender = tx_from_threads.clone();
    let passengers_thread_handle = thread::spawn(move || {
        let mut rejected_passengers: Vec<Passenger> = Vec::new();
        let receiver_from_sync_thread = rx_to_passengers;
        let mut previous_time_tick = 0;
        loop {
            // Wait for 1 milliseconds to give other threads a chance to use the time tick mutex
            std::thread::sleep(std::time::Duration::from_millis(1));
            let mut rejected_passengers_indeces: Vec<usize> = Vec::new();
            let time_tick = passenger_thread_time_tick_clone.lock().unwrap();
            println!("Passenger loop beginning. Time tick: {}", time_tick);

            if *passenger_thread_program_end_clone.lock().unwrap() {
                println!("Rejected Passenger count: {}", rejected_passengers.len());
                break;
            }

            if *time_tick == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                thread::yield_now();
                continue;
            } else if *time_tick % 2 == 0 || previous_time_tick == *time_tick {
                std::thread::sleep(std::time::Duration::from_millis(10));
                std::thread::yield_now();
                continue;
            }
            let bus_route_list = passenger_thread_bus_route_clone.lock().unwrap();
            let bus_route_list: Vec<Vec<PassengerBusLocation>> = bus_route_list
                .clone()
                .into_iter()
                .map(convert_bus_route_list_to_passenger_bus_route_list)
                .collect();

            if *time_tick == 1 {
                println!("First time tick loop");
                let mut passenger_list = passenger_thread_passenger_list_clone.lock().unwrap();
                println!("Beginning of tick one passenger calculations");
                for (passenger_index, passenger) in passenger_list.iter_mut().enumerate() {
                    match passenger.status {
                        PassengerStatus::Waiting => {
                            // If passenger is waiting for the bus, find out what bus will be able to take
                            // the passenger to his destination at a time later than this time step
                            // and send some message to the bus to pick him up
                            println!("Passenger Loop started on tick 1");

                            if let Ok(bus_schedule) = find_bus_to_pick_up_passenger(
                                passenger,
                                *time_tick,
                                bus_route_list.clone(),
                            ) {
                                passenger.bus_schedule = bus_schedule.clone();
                            } else {
                                println!("Some passengers were rejected");
                                rejected_passengers_indeces.push(passenger_index);
                            }
                            println!("Passenger Loop ended on tick 1");
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
                println!("End of tick one passenger calculations");

                // Remove passengers who cannot get onto a bus, since if they cannot get on any bus
                // now, they will not be able to later, because the schedules will not change. I
                // might as well continue keeping track of them.

                for passenger_index in rejected_passengers_indeces.into_iter().rev() {
                    let rejected_passenger = passenger_list.remove(passenger_index);
                    println!("Rejected passenger removed");
                    rejected_passengers.push(rejected_passenger);
                }

                passenger_thread_sender
                    .send(BusMessages::InitPassengers)
                    .unwrap();
                println!("Passengers init message sent");
                // break;
                assert_eq!(
                    passenger_list.len() + rejected_passengers.len(),
                    GLOBAL_PASSENGER_COUNT as usize
                );

                previous_time_tick = *time_tick;
                continue;
            }
            // Otherwise, the time tick is odd

            let mut passenger_list = passenger_thread_passenger_list_clone.lock().unwrap();
            println!("Passenger rejected thread time tick: {}", time_tick);

            // No message is sent, so the program halts

            // How do I prevent the time tick from blocking the program from running?
            // I use the time tick to on

            // drop time_tick so that the lock is released before waiting for a message
            drop(time_tick);

            let rejected_passenger_list_option = receiver_from_sync_thread.recv().unwrap();
            println!("Rejected Passenger Option: {rejected_passenger_list_option:?}");
            let time_tick = passenger_thread_time_tick_clone.lock().unwrap();

            println!("Rejected Bus Process Finished Message Received");
            if let Some(mut rejected_passenger_list) = rejected_passenger_list_option {
                // Somehow, the message only prints out once, yet around 490 passengers were rejected. Something is probably off.
                println!("Some passengers were rejected");
                let mut nonboardable_passengers_list = vec![];
                let mut nonboardable_passenger_indeces = vec![];
                println!("Passenger loop started");
                for passenger in rejected_passenger_list.iter_mut() {
                    match passenger.status {
                        PassengerStatus::Waiting => {
                            // If passenger is waiting for the bus, find out what bus will be able to take
                            // the passenger to his destination at a time later than this time step
                            // and send some message to the bus to pick him up

                            if let Ok(bus_schedule) = find_bus_to_pick_up_passenger(
                                passenger,
                                *time_tick,
                                bus_route_list.clone(),
                            ) {
                                passenger.bus_schedule = bus_schedule.clone();
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

                for nonboardable_passenger in nonboardable_passengers_list {
                    for (passenger_index, passenger) in passenger_list.iter().enumerate() {
                        if nonboardable_passenger == passenger {
                            nonboardable_passenger_indeces.push(passenger_index);
                            break;
                        }
                    }
                }

                nonboardable_passenger_indeces.sort();

                // FIXME: sorting the passenger indeces and reversing them should ensure
                // that the indeces continue to line up with the same passengers,
                // but something must be wrong beause occasionally, the thread panics
                // because an invalid index tries to be accessed
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
                println!(
                    "{finished_passenger_count} passengers sucessfully arived at their destination"
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
        }

        let total_rejected_passengers = rejected_passengers.len();
        println!("There were a total of {total_rejected_passengers} rejected passengers");
    });

    handle_list.push(passengers_thread_handle);

    for bus_num in 1..=NUM_OF_BUSES {
        let sender = tx_from_threads.clone();
        let bus_route = generate_bus_route_locations_with_distances(
            location_vector_arc.clone().as_ref(),
            NUM_STOPS_PER_BUS,
        )
        .unwrap();
        let passenger_list_pointer_clone = passenger_list_pointer.clone();
        let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let current_time_tick_clone = current_time_tick.clone();
        let mut time_clone_check = 1;
        let bus_route_vec_clone = bus_route_vec_arc.clone();
        let handle = thread::spawn(move || {
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY, bus_num);
            let mut bus_route_array = bus_route_vec_clone.lock().unwrap();
            bus_route_array[simulated_bus.bus_num - 1] = simulated_bus.get_bus_route();
            // Release the lock on bus_route_vec by dropping it
            drop(bus_route_array);
            sender
                .send(BusMessages::InitBus {
                    bus_number: simulated_bus.bus_num,
                })
                .unwrap();
            println!("Bus message sent");

            loop {
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
                    ControlFlow::Continue(UpdateOutput::WrongTimeTick) => {}
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
                    bus_number: simulated_bus.bus_num,
                })
                .unwrap();
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
    let passengers_remaining = GLOBAL_PASSENGER_COUNT - total_passengers as u32;

    println!("{passengers_remaining} passengers did not get onto any bus");

    println!("total extra stops waited: {}", total_extra_stops_waited);

    println!(
        "Average wait time between stops {}",
        total_extra_stops_waited as f64 / total_passengers as f64
    );
}
