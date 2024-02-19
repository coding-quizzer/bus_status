use bus_system::convert_bus_route_list_to_passenger_bus_route_list;
use bus_system::passenger::Passenger;
use bus_system::station::Station;
use bus_system::thread::BusThreadStatus;
use bus_system::thread::{
    BusMessages, RejectedPassengersMessages, StationMessages, StationToBusMessages,
    StationToPassengersMessages,
};
use bus_system::{generate_bus_route_locations_with_distances, generate_random_passenger_list};
use bus_system::{initialize_channel_list, initialize_location_list};
use std::collections::HashMap;
use std::{
    collections::VecDeque,
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use bus_system::bus::{Bus, BusLocation};
use bus_system::consts::*;
use bus_system::data::{self, InputDataStructure};
use bus_system::location::{Location, PassengerBusLocation};

fn main() {
    let initial_data: InputDataStructure = if READ_JSON {
        data::read_data_from_file(Path::new("bus_route_data.json")).unwrap()
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
    let location_vector: Vec<Location> = if READ_JSON {
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
                    let passenger_list: Vec<_> = route_sync_passenger_list_arc
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
                    data::write_data_to_file(json_structure, Path::new("bus_route_data.json"))
                        .unwrap();
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
                    GLOBAL_PASSENGER_COUNT
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
    for location in station_location_list.as_ref() {
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
            let current_receiver_with_index = station_channels.lock().unwrap().remove(0);
            let station_index = current_receiver_with_index.index;
            println!("Station index: {station_index}");
            let current_receiver = current_receiver_with_index.receiver;
            

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
                                    passenger.clone().into(),
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
                    StationMessages::BusArrived{passengers_onboarding, bus_info}=> {
                        for passenger in passengers_onboarding.clone().iter_mut() {
                          let passenger_location = passenger.bus_schedule_iterator.next().unwrap();
                          passenger.archived_stop_list.push(passenger_location);
                        }
                        println!("Passengers: {:?}", passengers_onboarding);
                        current_station.passengers.extend(passengers_onboarding);
                        let bus_index = bus_info.bus_index;
                        println!("Bus {bus_index} arrived at station {station_index}.");
                        current_station.docked_buses.push(bus_info);
                        (send_to_bus_channels.as_ref())[bus_index]
                            .send(StationToBusMessages::AcknowledgeArrival())
                            .unwrap();
                        

                        
                        }
                      }
                      let mut next_passengers_for_buses_hash_map = current_station.docked_buses.iter().map(|bus| (bus.bus_index, Vec::<Passenger>::new())).collect::<HashMap<_, _>>();

                      println!("Station {} Passengers:{:?}", station_index, current_station.passengers);
                      
                      // Somehow, bus needs to send passengers to currently docked buses

                      // TODO: Use a more efficient method than partition. Also, remove the clone, so peek actully gives an advantage.
                      let (passengers_for_next_destination, arrived_passengers): (Vec<_>, Vec<_>) = current_station.passengers.iter_mut().partition(| passenger| passenger.bus_schedule_iterator.clone().peek().is_none());
                      // println!("Passengers for next destination: {:?}", &passengers_for_next_destination);
                      let mut remaining_passengers: Vec<Passenger> = Vec::new();
                      // overflowed passengers have their own list so that they can be recalculated
                      let mut passengers_overflowed = Vec::new();
                      // println!("Arrived Passengers: {:?}", &arrived_passengers);
                      println!("Passengers for next destination: {:?}", passengers_for_next_destination);
                      for passenger in passengers_for_next_destination {
                        println!("passenger_loop");
                        // Does this work, or will this be the next next location?
                        let current_location = passenger.bus_schedule_iterator.peek().unwrap();
                        let next_bus_index = current_location.bus_num.expect("Since there is a location after this, the next bus index should not be null");
                        
                      
                        if next_passengers_for_buses_hash_map.contains_key(&next_bus_index) {
                          next_passengers_for_buses_hash_map.entry(next_bus_index).and_modify(|passenger_list| passenger_list.push(passenger.clone()));
                        } else {remaining_passengers.push(passenger.clone())}

                        dbg!(&next_passengers_for_buses_hash_map);
                        
                        
                      }
                      
                      for bus in &(current_station.docked_buses) {
                        let mut passengers_to_send = Vec::new();
                        let bus_index = bus.bus_index;
                        let new_passenger_list_entry = next_passengers_for_buses_hash_map.entry(bus_index);
                        let remaining_capacity = bus.capacity_remaining;
                        let new_passenger_list = new_passenger_list_entry.or_default();
                        if new_passenger_list.len() > remaining_capacity {
                          let (passengers_to_add, rejected_passengers) = new_passenger_list.split_at(remaining_capacity);
                          passengers_overflowed.append(rejected_passengers.to_vec().as_mut());
                          passengers_to_send.append(passengers_to_add.to_vec().as_mut());
                        } else {
                          passengers_to_send.append(new_passenger_list);
                        }

                        println!("Passengers to send: {:?}", passengers_to_send);

                        send_to_bus_channels.as_ref()[bus_index].send(StationToBusMessages::SendPassengers(passengers_to_send)).unwrap();


                        // deal with overflowed passengers
                        // Remove departed buses from the current time tick from the bus list


                      }
                      // current_station.passengers = passengers_for_next_destination;

            

              }
        });

        handle_list.push(station_handle);
    }

    for _ in 0..NUM_OF_BUSES {
        let bus_route_array_clone = bus_route_vec_arc.clone();
        let station_senders_clone = send_to_station_channels_arc.clone();
        let sender = tx_from_bus_threads.clone();
        let bus_receiver_channels = bus_receiver_channels_arc.clone();

        // let passenger_list_pointer_clone = passenger_list_pointer.clone();
        // let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let current_time_tick_clone = current_time_tick.clone();
        let mut time_clone_check = 1;
        let handle = thread::spawn(move || {
            let current_bus_receiver_with_index = bus_receiver_channels.lock().unwrap().remove(0);
            drop(bus_receiver_channels);
            // Since the receiver obtained is not deterministic, set the bus index based on what what channel was obtained
            let bus_index = current_bus_receiver_with_index.index;
            println!("Bus index: {}", bus_index);
            let bus_receiver = current_bus_receiver_with_index.receiver;
            let mut bus_route_array = bus_route_array_clone.lock().unwrap();
            let bus_route = bus_route_array.get(bus_index).unwrap();
            println!("Bus {bus_index} bus route: {bus_route:#?}");
            let mut simulated_bus = Bus::new(bus_route.clone(), BUS_CAPACITY, bus_index);
            println!("Bus {bus_index} created");
            bus_route_array[simulated_bus.bus_index] = simulated_bus.get_bus_route();
            // Release the lock on bus_route_array by dropping it
            drop(bus_route_array);
            sender
                .send(BusMessages::InitBus {
                    bus_index: simulated_bus.bus_index,
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
                let time_tick = *current_time_tick;

                drop(current_time_tick);

                simulated_bus.update(station_senders_clone.as_ref(), &bus_receiver, &sender, &time_tick);
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
