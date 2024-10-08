use crate::bus::{BusLocation, SendableBus};
use crate::consts::DEFAULT_NUM_OF_BUSES;
use crate::location::{Location, PassengerBusLocation};
use crate::main_loop::FinalPassengerLists;
use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;
use crate::thread::{
    StationMessages, StationToBusMessages, StationToPassengersMessages, SyncToStationMessages,
};
use crate::TimeTick;
use crate::{
    calculate_passenger_schedule_for_bus,
    calculate_passenger_schedule_for_bus_check_available_buses,
};
use crate::{ReceiverWithIndex, TimeTickStage};
use core::time;
use std::os::linux::raw::stat;
use std::sync::mpsc::TryRecvError;
use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};

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

use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub struct Station {
    pub location: Location,
    pub docked_buses: Vec<SendableBus>,
    pub passengers: Vec<Passenger>,
    pub buses_unavailable: Vec<usize>,
    pub bus_loading_first_iteration: Option<bool>,
    pub arrived_passengers: Vec<Passenger>,
}

impl Station {
    pub fn new(location: Location, passenger_bus_routes: &Vec<Vec<PassengerBusLocation>>) -> Self {
        let location_time_tick_hashmap =
            get_station_buses_index_hash_map(location.index, passenger_bus_routes);
        // the passenger bus route may not have the bus route yet.
        dbg!(&location_time_tick_hashmap);
        Station {
            location,
            docked_buses: Vec::new(),
            passengers: Vec::new(),
            buses_unavailable: Vec::new(),
            bus_loading_first_iteration: None,
            arrived_passengers: Vec::new(),
        }
    }

    pub fn add_passenger(
        &mut self,
        mut new_passenger: Passenger,
        time_tick: TimeTick,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
    ) -> Result<(), Passenger> {
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule =
            calculate_passenger_schedule_for_bus(&new_passenger, time_tick.number, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule_iterator = new_bus_schedule.clone().into_iter().peekable();
        new_passenger.bus_schedule = new_bus_schedule;

        let first_passenger_location = new_passenger
            .bus_schedule_iterator
            .next()
            .expect("If the passenger has a route, the passenger will a first location");
        new_passenger.next_bus_num = first_passenger_location.bus_num;
        // The current location does not need setting because it is already set
        self.passengers.push(new_passenger);
        Ok(())
    }

    pub fn add_passenger_check_available_buses(
        &mut self,
        mut new_passenger: Passenger,
        current_time_tick: TimeTick,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
        buses_unavailable: Vec<usize>,
    ) -> Result<(), Passenger> {
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule = calculate_passenger_schedule_for_bus_check_available_buses(
            &new_passenger,
            current_time_tick.number,
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

/// Function to calculate what buses will arrive at the station on a given time tick
/// debug statements are empty, so the bus routes must not have a value at this point
pub fn get_station_buses_index_hash_map(
    station_index: usize,
    bus_routes: &Vec<Vec<PassengerBusLocation>>,
    // time_tick: u32,
) -> HashMap<u32, Vec<usize>> {
    let mut route_hash_map = HashMap::new();
    dbg!(bus_routes);
    for (bus_index, route) in bus_routes.iter().enumerate() {
        for bus_location in route {
            if bus_location.location.index == station_index {
                route_hash_map
                    .entry(bus_location.location_time_tick)
                    .and_modify(|bus_list: &mut Vec<usize>| bus_list.push(bus_index))
                    .or_insert(vec![bus_index]);
            }
        }
    }

    route_hash_map
}

///
///
pub fn get_station_threads(
    station_location_list: &Vec<Location>,
    current_time_tick: &TimeTick,
    send_to_bus_channels_arc: &Arc<Vec<Sender<StationToBusMessages>>>,
    receive_in_station_channels_arc: &Arc<Mutex<Vec<Option<ReceiverWithIndex<StationMessages>>>>>,
    bus_route_vec_arc: &Arc<Mutex<Vec<Vec<BusLocation>>>>,
    passenger_bus_route_arc: &Arc<Mutex<Vec<Vec<PassengerBusLocation>>>>,
    rejected_passengers_pointer: &Arc<Mutex<Vec<Passenger>>>,

    tx_stations_to_passengers: Sender<StationToPassengersMessages>,
    mut rx_sync_to_stations_list: Arc<Mutex<Vec<Option<ReceiverWithIndex<SyncToStationMessages>>>>>,
    final_passenger_list_arc: &Arc<Mutex<FinalPassengerLists>>,
) -> Vec<JoinHandle<()>> {
    // let station_location_list = location_vector_arc.clone();

    let mut handle_list = Vec::new();

    // Station index and location index are equivilant
    for location in station_location_list {
        let station_index = location.index;
        let sync_to_stations_reciever = rx_sync_to_stations_list.clone().lock().unwrap()
            [station_index]
            .take()
            .expect("Station index in sync_to_stations list should still be available")
            .receiver;
        let station_channel = receive_in_station_channels_arc.clone().lock().unwrap()
            [station_index]
            .take()
            .expect("Station index in station_receivers list should still be available");
        let station_time_tick = current_time_tick;
        let current_location = *location;
        let send_to_bus_channels = send_to_bus_channels_arc.clone();
        let station_channels = receive_in_station_channels_arc.clone();
        let bus_route_list = bus_route_vec_arc.clone();
        let station_thread_passenger_bus_route_list = passenger_bus_route_arc.clone();
        // let send_to_bus_channels =
        let rejected_passenger_clone = rejected_passengers_pointer.clone();
        let final_passenger_list_clone = final_passenger_list_arc.clone();

        let to_passengers_sender_clone = tx_stations_to_passengers.clone();
        let station_handle = create_station_thread(
            current_location,
            station_time_tick.clone(),
            send_to_bus_channels,
            station_channel,
            bus_route_list,
            station_thread_passenger_bus_route_list,
            rejected_passenger_clone,
            to_passengers_sender_clone,
            sync_to_stations_reciever,
            final_passenger_list_clone,
        );
        handle_list.push(station_handle);
    }
    handle_list
}

pub fn create_station_thread(
    current_location: Location,
    station_time_tick: TimeTick,
    send_to_bus_channels: Arc<Vec<Sender<StationToBusMessages>>>,
    station_channel_receiver: ReceiverWithIndex<StationMessages>,
    bus_route_list: Arc<Mutex<Vec<Vec<BusLocation>>>>,
    station_thread_passenger_bus_route_list: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>>,
    rejected_passenger_clone: Arc<Mutex<Vec<Passenger>>>,
    to_passengers_sender_clone: Sender<StationToPassengersMessages>,
    sync_to_stations_receiver: Receiver<SyncToStationMessages>,
    final_passenger_list_clone: Arc<Mutex<FinalPassengerLists>>,
) -> JoinHandle<()> {
    let station_handle = thread::spawn(move || {
        let mut current_station = Station::new(
            current_location,
            &station_thread_passenger_bus_route_list.lock().unwrap(),
        );
        let station_index = station_channel_receiver.index;
        println!("Station index: {station_index}");
        let current_receiver = station_channel_receiver.receiver;
        // Once passengerInit stage is finished, bus_passengers_initialized is set to false, so the loop does not need to run again
        let mut bus_passengers_initialized = false;

        'main: loop {
            if let Ok(SyncToStationMessages::ProgramFinished(
                crate::thread::ProgramEndType::ProgramFinished,
            )) = sync_to_stations_receiver.try_recv()
            {
                println!(
                    "All buses finished message received in station {}",
                    station_index
                );
                let mut final_passenger_list = final_passenger_list_clone.lock().unwrap();
                final_passenger_list.location_lists[station_index] =
                    current_station.arrived_passengers;
                break;
            }
            // println!("Station {location_index}. time tick: {}", *time_tick);

            let time_tick = station_time_tick;
            /* println!(
                "Station {} loop beginning. Time tick: {:?}",
                station_index, time_tick
            ); */
            // Note: It may be a good idea to make sure that stations without passengers are not blocked, but this might work

            // time tick is kept by deadlock

            // let received_message = current_receiver.recv().unwrap();

            match time_tick.stage {
                TimeTickStage::PassengerInit => {
                    if bus_passengers_initialized {
                        drop(time_tick);
                        continue;
                    }

                    println!("Station {} first timetick", station_index);
                    let received_message = current_receiver.recv().unwrap();

                    println!(
                        "Station {} first message received: {:#?}",
                        station_index, received_message
                    );

                    // If I change this to be a new reciever, the other stages will not need to filter out this option
                    if let StationMessages::InitPassengerList(mut list) = received_message {
                        assert_eq!(time_tick.number, 0);
                        println!("Station: {station_index} Message: {list:#?}");
                        let mut rejected_passenger_indeces = Vec::new();
                        println!("List: {:?}", list);
                        for (index, passenger) in list.iter().enumerate() {
                            // let passenger_bus_route_list =
                            //     station_thread_passenger_bus_route_list.lock().unwrap();
                            println!("Passenger will be added to station {}", station_index);
                            current_station
                                .add_passenger(
                                    passenger.clone().into(),
                                    time_tick,
                                    &station_thread_passenger_bus_route_list.lock().unwrap(),
                                )
                                .unwrap_or_else(|passenger| {
                                    rejected_passenger_indeces.push(index);
                                    println!("Passenger {:?} had no valid routes", passenger);
                                });
                        }

                        drop(time_tick);

                        for passenger_index in rejected_passenger_indeces.into_iter().rev() {
                            let removed_passenger = list.remove(passenger_index);
                            (*rejected_passenger_clone.lock().unwrap()).push(removed_passenger);
                        }
                        println!("Passengers added to station {}", station_index);
                        // drop(time_tick);
                        println!("total passenger_count: {}", list.len());
                        to_passengers_sender_clone
                            .send(StationToPassengersMessages::ConfirmInitPassengerList(
                                station_index,
                            ))
                            .unwrap();
                    } else {
                        let time_tick = station_time_tick;

                        panic!(
                            "Invalid Message:{:?} for present timetick: {:?}. Expected InitPassengerList ",
                            received_message, time_tick
                        )
                    }
                    bus_passengers_initialized = true;
                }

                TimeTickStage::BusUnloadingPassengers => {
                    /* println!(
                        "Station {} running BusUnloadingPassenger timestep.",
                        station_index
                    ); */

                    // Implimentation
                    // The previous loading stage should be finished, so bus_loading_first_iteration should be set to none
                    assert!(current_station.bus_loading_first_iteration.is_none());

                    // There is an indeterminate number of buses stopping at this stage. The received message should probably use try_recv to only take bus messages as they come and not wait for a message that is not coming
                    let received_message = current_receiver.try_recv();
                    let received_message = match received_message {
                        Ok(message) => {
                            // I set up an infinite loop in BusLoading passengers and the break statement was never hit. How did the program get here on time tick 2?
                            println!(
                                "Station {} received message {:?} on BusUnloadingPassengers stage received on time tick {:?}", current_location.index , message, time_tick
                            );
                            message
                        }
                        Err(TryRecvError::Empty) => {
                            continue;
                        }
                        Err(TryRecvError::Disconnected) => {
                            panic!("{}", TryRecvError::Disconnected);
                        }
                    };
                    drop(time_tick);
                    println!(
                        "Station {} BusUnloadingPassengers timetick stage",
                        station_index
                    );

                    println!(
                        "station {} received message received on BusUnloadingPassengers stage",
                        station_index
                    );

                    // On some runs, the received message is BusDeparted, for some reason

                    // THe bus keeps leaving and then returning again. The the passengers are not duplicated in the bus and the bus is not duplicated in the station.

                    // DEBUG: This message is sometimes not received. But it does seem to consistantly be sent. What is happening?
                    if let StationMessages::BusArrived {
                        mut passengers_onboarding,
                        bus_info,
                    } = received_message
                    {
                        println!(
                            "Bus Arrived Message received in station {} from bus {}",
                            station_index, bus_info.bus_index
                        );
                        for passenger in passengers_onboarding.iter_mut() {
                            // TODO: These opperations might be redundant, or should be done with Station::add_passenger. Explore this further
                            let passenger_location =
                                passenger.bus_schedule_iterator.next().unwrap();
                            passenger.current_location = passenger_location.stop_location.into();
                            passenger.next_bus_num = passenger_location.bus_num;
                            passenger.archived_stop_list.push(passenger_location);
                        }
                        println!(
                            "Passengers Onboarding from bus {:?}: {:?}",
                            bus_info.bus_index, passengers_onboarding
                        );
                        current_station.passengers.extend(passengers_onboarding);
                        let bus_index = bus_info.bus_index;
                        println!("Bus {bus_index} arrived at station {station_index}. Received from station");

                        assert!(!current_station
                            .docked_buses
                            .iter()
                            .any(|station_bus| station_bus == &bus_info));

                        current_station.docked_buses.push(bus_info);

                        //FIXME: Two consecutive sends to the same thread without any regulation of timing

                        // So far, acknowledge arrival doesn't acctually do anything
                        // send_to_bus_channels[bus_index]
                        //     .send(StationToBusMessages::AcknowledgeArrival())
                        //     .unwrap();

                        send_to_bus_channels[bus_index]
                            .send(StationToBusMessages::FinishedUnloading)
                            .unwrap();
                        println!(
                            "Station {} finished unloading bus {} and sent message",
                            station_index, bus_index
                        );
                    } else {
                        let time_tick = station_time_tick;

                        panic!(
                            "Invalid Message:{:?} for present timetick: {:?}. Expected BusArrived ",
                            received_message, time_tick
                        )
                    }
                }
                TimeTickStage::BusLoadingPassengers { .. } => {
                    println!("BusLoadingPassengers timetick beginning.");
                    // New pasted material
                    println!(
                        "Is first iteration: {:?}. Station: {}. Time tick: {:?}",
                        current_station.bus_loading_first_iteration,
                        current_location.index,
                        time_tick
                    );
                    if current_station.bus_loading_first_iteration.is_none() {
                        current_station.bus_loading_first_iteration = Some(true)
                    };
                    // This should only happen within the first iteration. Afterwords, it should be caught in the loop
                    assert!(current_station.bus_loading_first_iteration.unwrap());

                    // This time tick was dropped and reinitialized so that the time tick is only mutable when neccessary

                    //let time_tick = station_time_tick.lock().unwrap();

                    // An iterator containing tuples containing the bus_index of each docked bus and a list of passengers that will get on that bus
                    let mut docked_bus_passenger_pairs_iter = current_station
                        .docked_buses
                        .iter()
                        .map(|bus| (bus.bus_index, Vec::<Passenger>::new()));

                    // Contains the next bus each waiting passenger will get on next
                    let mut next_passengers_for_buses_array: [Option<Vec<Passenger>>;
                        DEFAULT_NUM_OF_BUSES] = std::array::from_fn(|_| None);

                    println!(
                        "Array with locations for station {:?}: {:?}",
                        &current_station.location.index, &docked_bus_passenger_pairs_iter
                    );

                    let mut docked_bus_passenger_pairs_vec =
                        docked_bus_passenger_pairs_iter.collect::<Vec<_>>();

                    // TODO: Improve adding to the list so that it does not need to be sorted
                    docked_bus_passenger_pairs_vec
                        .sort_by(|bus_prev, bus_next| bus_prev.0.cmp(&bus_next.0));

                    let mut docked_bus_passenger_pairs_iter =
                        docked_bus_passenger_pairs_vec.into_iter();

                    let mut next_vec = docked_bus_passenger_pairs_iter.next();
                    println!("Station {} next vec: {:?}", station_index, next_vec);

                    // Add empty arrays at the indeces of buses docked at the station
                    next_passengers_for_buses_array = next_passengers_for_buses_array
                        .into_iter()
                        .enumerate()
                        .map(|(current_index, _)| {
                            if let Some((old_index, vector)) = &mut next_vec {
                                if &current_index == old_index {
                                    let next_vec_vector = std::mem::take(vector);
                                    next_vec = docked_bus_passenger_pairs_iter.next();
                                    Some(next_vec_vector)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();

                    println!(
                        "station {} next passengers for buses array: {:?}",
                        station_index, next_passengers_for_buses_array
                    );

                    // dbg!(&next_passengers_for_buses_array);

                    // FIXME: Why are some passengers at the station when they should be on the bus?
                    // (For example, passengers going from station 3 to station 4 should be picked up
                    // from station 3 at time tick 2, but are still in station 3 at time tick 4)

                    // I don't think dropped off passengers are removed from the station - that's an obvious problem
                    println!(
                        "Station {} Passengers: {:#?}",
                        current_station.location.index, current_station.passengers
                    );

                    // Somehow, bus needs to send passengers to currently docked buses

                    // TODO: Use a more efficient method than partition. Also, remove the clone, so peek actully gives an advantage.
                    // I feel like there may not be enough cases taken in consideration
                    // Some passengers that are arrived and have this as the final destination are still listed under passengers_for_next_destination, This filter is not working correctly
                    let (passengers_for_next_destination, arrived_passengers): (Vec<_>, Vec<_>) =
                        current_station
                            .passengers
                            .iter_mut()
                            .partition(|passenger| {
                                let mut passenger_iterator_clone =
                                    passenger.bus_schedule_iterator.clone();
                                // The next location will be the next station, which should be None if this is the last one
                                // This will be some, given the next location represents the current station
                                let next_location = passenger_iterator_clone.next();
                                println!("Passenger schedule: {:#?}", passenger.bus_schedule);
                                println!("Passenger, {:#?}", passenger);
                                println!("Current location number: {}", station_index);
                                println!("Next location {:#?}", next_location);
                                println!("Time tick: {:?}", time_tick);

                                next_location.is_some()
                            });
                    // ensure this is actually arrived passengers have actually arrived at the correct destination
                    println!(
                        "Time_tick: {}, Station {} Arrived Passengers: {:#?}",
                        time_tick.number, station_index, arrived_passengers
                    );
                    assert!(arrived_passengers
                        .iter()
                        .all(|passenger| passenger.destination_location == current_location));

                    // println!("Passengers for next destination: {:?}", &passengers_for_next_destination);;

                    let mut remaining_passengers: Vec<Passenger> = Vec::new();
                    // overflowed passengers have their own list so that they can be recalculated
                    let mut passengers_overflowed: Vec<Passenger> = Vec::new();
                    // println!("Arrived Passengers: {:?}", &arrived_passengers);
                    // FIXME: Some passengers are showing up at the station when they should be on the bus. How does this work?
                    println!(
                        "Station {} Passengers for next destination: {:#?}",
                        station_index, passengers_for_next_destination
                    );
                    for passenger in passengers_for_next_destination {
                        println!("passenger_loop");
                        // Does this work, or will this be the next next location?
                        let next_bus_index = passenger.next_bus_num.expect(
                          "Since there is a location after this, the next bus index should not be None",
                            );

                        if let Some(ref mut passengers) =
                            next_passengers_for_buses_array[next_bus_index]
                        {
                            passengers.push(passenger.clone());
                        } else {
                            remaining_passengers.push(passenger.clone());
                        }
                    }

                    dbg!(&next_passengers_for_buses_array);
                    current_station.passengers = remaining_passengers;

                    // End of newly pasted code

                    let docked_buses = &(current_station.docked_buses.clone());

                    for bus in docked_buses {
                        // println!("Station loop beginning.");
                        println!("Station Bus Time tick: {:?}", time_tick);
                        println!("Station: {}", current_station.location.index);
                        println!("Bus: {}", bus.bus_index);
                        let mut passengers_to_send = Vec::new();
                        let bus_index = bus.bus_index;
                        let remaining_capacity = bus.capacity_remaining;
                        // The index does not exist in the array - even though the index should be of a docked bus
                        // let passengers_overflowed: Vec<_> = todo!();

                        println!(
                            "Next passengers for buses, {:?}. Bus index: {}",
                            next_passengers_for_buses_array, bus_index,
                        );

                        // Usually time tick misfunction
                        let mut new_passenger_list = next_passengers_for_buses_array[bus_index]
                            .clone()
                            .unwrap_or_else(|| {
                                panic!(
                                    "Bus {bus_index} should be docked at the station {}",
                                    current_station.location.index
                                )
                            });
                        if new_passenger_list.len() > remaining_capacity {
                            let (passengers_to_add, rejected_passengers) =
                                new_passenger_list.split_at(remaining_capacity);
                            passengers_overflowed.append(rejected_passengers.to_vec().as_mut());
                            passengers_to_send.append(passengers_to_add.to_vec().as_mut());
                            current_station.buses_unavailable.push(bus.bus_index);
                        } else {
                            passengers_to_send.append(&mut new_passenger_list);
                        }
                        // FIXME: Some of these passengers are sent to the same bus both from the beginning station and from the end station
                        println!(
                            "Passengers to send from station {}: {:#?}",
                            station_index, passengers_to_send,
                        );

                        send_to_bus_channels[bus_index]
                            .send(StationToBusMessages::SendPassengers(passengers_to_send))
                            .unwrap();

                        let current_bus_route_list = bus_route_list.lock().unwrap();
                        let bus_route_vec: Vec<_> =
                            current_bus_route_list.clone().into_iter().collect();
                        // bus_route_vec[0][0].

                        drop(current_bus_route_list);
                        println!("Current bus route list dropped");

                        // TODO: Deal with passengers without an available route
                        for passenger in passengers_overflowed.clone() {
                            let unavailable_buses = current_station.buses_unavailable.clone();
                            current_station
                                .add_passenger_check_available_buses(
                                    passenger,
                                    time_tick,
                                    &station_thread_passenger_bus_route_list.lock().unwrap(),
                                    unavailable_buses,
                                )
                                .unwrap();
                        }

                        // drop(time_tick);

                        println!("Pre-bus departure");
                    }

                    println!("Time tick when buses are dismissed: {:?}", &time_tick);
                    for bus in current_station.docked_buses.iter() {
                        send_to_bus_channels[bus.bus_index]
                            .send(StationToBusMessages::RequestDeparture)
                            .unwrap();
                    }

                    println!("Departure message sent");

                    current_station.bus_loading_first_iteration = Some(false);

                    // Occasionally, this turns into an infinite loop

                    drop(time_tick);
                    // Why is this not running on time tick 1?
                    'bus_loading: loop {
                        // For test data, program is stuck in this loop, even though the time step has proceeded to the next one
                        println!(
                            "Bus loading loop beginning in station {} at time tick {:?}",
                            current_location.index, station_time_tick
                        );
                        // sync_to_stations receiver moved before buses_receiver so that this check can run independantly of
                        // other messages.
                        if let Ok(SyncToStationMessages::AdvanceTimeStep(prev_time_tick)) =
                            sync_to_stations_receiver.try_recv()
                        {
                            // By the time this message is received, two time tick iterations have gone by.
                            // I need to prevent the station from going on to a new stage before the increment time tick method is declared

                            println!(
                                "Advance Time tick message received in station {}",
                                station_index
                            );
                            println!("Time tick: {:?}", station_time_tick);
                            // At this point if all the buses have finished their tick, they should be gone from the station
                            let time_tick = station_time_tick;
                            println!("Time tick locked on Station bus_loading loop");

                            if !(current_station.docked_buses.is_empty()) {
                                println!("Station {} time ticks", current_location.index);
                                println!("Time tick before incrementing: {:?}", prev_time_tick);
                                println!("Current time tick: {:?}", time_tick);
                                panic!(
                                    "Station {} still contains buses: {:?}",
                                    current_station.location.index, current_station.docked_buses
                                );
                            };

                            println!(
                                "All Buses departed from station {} at time tick {:?}.",
                                current_station.location.index, time_tick,
                            );

                            // Clear buses unavailable list
                            current_station.buses_unavailable = Vec::new();
                            println!(
                                "Station received AdvanceTimeStep message at time tick {:?}",
                                time_tick
                            );
                            current_station.bus_loading_first_iteration = None;
                            break 'bus_loading;
                        }

                        let time_tick = station_time_tick;

                        let received_message = current_receiver.try_recv();
                        let received_message = match received_message {
                            Ok(message) => message,
                            Err(TryRecvError::Empty) => continue 'bus_loading,
                            Err(TryRecvError::Disconnected) => {
                                panic!("{}", TryRecvError::Disconnected)
                            }
                        };

                        if let StationMessages::BusDeparted { bus_index } = received_message {
                            drop(time_tick);

                            // Confirm that the station contains the bus that should be removed
                            assert!(current_station
                                .docked_buses
                                .iter()
                                .any(|bus| bus.bus_index == bus_index));

                            current_station
                                .docked_buses
                                .retain(|bus| bus.bus_index != bus_index);

                            // How is the bus departure received before going through this?
                            println!(
                                "Bus {} removed from station {}",
                                bus_index, current_location.index
                            );
                            send_to_bus_channels[bus_index]
                                .send(StationToBusMessages::StationRemovedBus)
                                .unwrap();
                        }
                    }
                    // bus_loading_first_iteration was set to None before breaking, so it should remain None
                    assert!(current_station.bus_loading_first_iteration.is_none());
                    println!(
                        "Bus Loading Loop finished in station {}",
                        current_station.location.index
                    );
                }
            }
        }
    });

    station_handle
}
