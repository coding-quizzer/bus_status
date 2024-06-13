use crate::bus::Bus;
use crate::consts::{
    BUS_CAPACITY, GLOBAL_LOCATION_COUNT, GLOBAL_PASSENGER_COUNT, NUM_OF_BUSES, WRITE_JSON,
};
use crate::data;
use crate::data::InputDataStructure;
use crate::initialize_channel_list;
use crate::location::{BusLocation, PassengerBusLocation};
use crate::station;
use crate::thread::{
    BusMessages, BusThreadStatus, StationMessages, StationToPassengersMessages,
    SyncToStationMessages,
};
use crate::{Location, Passenger};
use crate::{TimeTick, TimeTickStage};

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::{self, mpsc, Arc, Mutex};
pub fn main_loop(
    location_vector: Vec<Location>,
    total_passenger_list: Vec<Passenger>,
    bus_route_array: [Vec<BusLocation>; NUM_OF_BUSES],
) {
    let passenger_bus_route_list: Vec<_> = bus_route_array
        .clone()
        .into_iter()
        .map(crate::convert_bus_route_list_to_passenger_bus_route_list)
        .collect();

    let rejected_passengers_pointer = Arc::new(Mutex::new(Vec::new()));

    let passenger_list_pointer = Arc::new(Mutex::new(total_passenger_list));

    let location_vector_arc = Arc::new(location_vector);

    let bus_route_vec_arc: Arc<Mutex<[Vec<BusLocation>; NUM_OF_BUSES]>> =
        Arc::new(Mutex::new(bus_route_array));

    let passenger_bus_route_arc: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>> =
        Arc::new(Mutex::new(passenger_bus_route_list));

    let passenger_extra_stops_waited_pointer = Arc::new(Mutex::new(Vec::<u32>::new()));

    // Split time ticks into two - time ticks are accurate
    let current_time_tick = Arc::new(Mutex::new(TimeTick::default()));

    let program_end = Arc::new(Mutex::new(false));

    let mut handle_list = vec![];

    let sync_handle_program_end_clone = program_end.clone();

    let (tx_from_bus_threads, rx_from_threads) = mpsc::channel();

    let (tx_to_passengers, rx_to_passengers) = mpsc::channel();

    let (tx_stations_to_passengers, mut rx_stations_to_passengers) = mpsc::channel();
    let (send_to_station_channels, receive_in_station_channels) =
        crate::initialize_channel_list(GLOBAL_LOCATION_COUNT);
    let receive_in_station_channels: Vec<_> =
        receive_in_station_channels.into_iter().map(Some).collect();

    let (sender_sync_to_stations_list, receiver_sync_to_stations_list) =
        initialize_channel_list::<SyncToStationMessages>(GLOBAL_LOCATION_COUNT);

    let receiver_sync_to_stations_list: Vec<_> = receiver_sync_to_stations_list
        .into_iter()
        .map(|receiver| Some(receiver))
        .collect();
    let sync_to_stations_receiver = Arc::new(Mutex::new(receiver_sync_to_stations_list));

    let send_to_station_channels_arc = Arc::new(send_to_station_channels);
    let receive_in_station_channels_arc = Arc::new(Mutex::new(receive_in_station_channels));

    let (send_to_bus_channels, receive_in_bus_channels) = initialize_channel_list(NUM_OF_BUSES);

    let bus_receiver_channels_arc = Arc::new(Mutex::new(receive_in_bus_channels));
    let send_to_bus_channels_arc = Arc::new(send_to_bus_channels);

    let current_time_tick_clone = current_time_tick.clone();
    #[track_caller]
    fn increment_and_drop_time_step(
        mut time_tick: sync::MutexGuard<'_, TimeTick>,
        station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
    ) {
        let call_location = std::panic::Location::caller();
        println!("---------- All buses are finished at their stops -----------");
        println!("Time step incremented from {}", call_location);
        // At this point sending the message here is not neccesary.
        // Because the time step mutiex is held for both time step increments,
        // the station will be stuck in the unloading phase when all the buses are moving
        // Eventually a message might be neccesary for that case but this implimentation works for now
        /*
          // This message will only be sent after the bus unloading stage is finished
            station_sender
              .send(SyncToStationMessages::AdvanceTimeStep(*time_tick))
              .unwrap();
        */

        (*time_tick).increment_time_tick();
    }

    fn manage_time_tick_increase_for_finished_loading_tick(
        mut current_time_tick: sync::MutexGuard<'_, TimeTick>,
        station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
        bus_status_array: &mut [BusThreadStatus; NUM_OF_BUSES],
    ) {
        println!("Bus statuses: {:?}", &bus_status_array);
        // The bus_loading timestep is finished, so the array is reset the entire status array
        for status in bus_status_array.iter_mut() {
            // Reset the statuses for the next time step
            if status == &BusThreadStatus::FinishedLoadingPassengers
                || status == &BusThreadStatus::Moving
            {
                *status = BusThreadStatus::WaitingForTimeStep;
            }
        }
        increment_and_drop_time_step(current_time_tick, station_senders);

        println!("End of sync loop");
    }

    let route_sync_bus_route_vec_arc = bus_route_vec_arc.clone();
    let route_sync_passenger_list_arc = passenger_list_pointer.clone();
    let route_sync_location_vec_arc = location_vector_arc.clone();

    let route_sync_handle = std::thread::spawn(move || {
        let station_senders = sender_sync_to_stations_list;
        let passenger_sender = tx_to_passengers;
        let mut bus_status_array = [BusThreadStatus::Uninitialized; NUM_OF_BUSES];
        // Bus that has unloaded passengers/ moving buses
        let mut processed_bus_received_count = 0;
        let mut rejected_passengers_list: Vec<Passenger> = Vec::new();
        let mut processed_moving_bus_count = 0;

        // Eventually, there will be two time ticks per movement so that the stopped buses can have two ticks:
        // One for unloading passengers and another for loading them. Bus Movement will probably happen on the second tick
        // (Uninitialized items could contain the empty vector)

        // Buses are initialized on the first time tick, Passengers choose their bus route info on the second time tick, and the bus route happens on the third time tick

        loop {
            println!("Route sync loop beginning");
            println!("processed bus received count: {processed_bus_received_count}");
            //
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
            println!("Bus status list: {:?}", bus_status_array);
            println!("Before receiving a message.");
            let received_bus_stop_message = rx_from_threads.recv().unwrap();

            println!("Message received: {:?}", received_bus_stop_message);
            let mut current_time_tick = current_time_tick_clone.lock().unwrap();
            println!("Message Received. Time tick: {:?}", current_time_tick);
            println!(
                "Before time processing, bus processed count: {}",
                processed_bus_received_count
            );
            match received_bus_stop_message {
                BusMessages::AdvanceTimeStepForUnloadedBus { bus_index } => {
                    bus_status_array[bus_index] = BusThreadStatus::FinishedUnloadingPassengers;
                }

                BusMessages::AdvanceTimeStepForLoadedBus { bus_index } => {
                    bus_status_array[bus_index] = BusThreadStatus::FinishedLoadingPassengers;
                }

                BusMessages::AdvanceTimeStepForMovingBus { bus_index } => {
                    bus_status_array[bus_index] = BusThreadStatus::Moving;
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
                    println!("Passenger initialized");
                    // current_time_tick.increment_time_tick();
                    // This ensures the command only runs once,
                    current_time_tick.increment_from_initialized();
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

            if current_time_tick.stage == TimeTickStage::PassengerInit
                && bus_status_array
                    .iter()
                    .filter(|status| *status == &BusThreadStatus::Uninitialized)
                    .count()
                    == 0
            {
                // This occasionally runs before all buses have received passengers
                println!("All buses initialized in sync thread");
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
                    data::write_data_to_file(
                        json_structure,
                        std::path::Path::new("bus_route_data.json"),
                    )
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
                for sender in station_senders {
                    sender
                        .send(SyncToStationMessages::BusRoutesFinished)
                        .unwrap();
                }

                println!("Program Complete");
                break;
            }

            // Increment the time step after all buses have run
            // if std::ops::Not::not(
            //     bus_status_array
            //         .iter()
            //         .any(|status| status == &BusThreadStatus::WaitingForTimeStep),
            // )

            const LOADING_BUS_VALID_STATUSES: [BusThreadStatus; 3] = [
                BusThreadStatus::BusFinishedRoute,
                BusThreadStatus::FinishedLoadingPassengers,
                BusThreadStatus::Moving,
            ];

            const UNLOADING_BUS_VALID_STATUSES: [BusThreadStatus; 3] = [
                BusThreadStatus::BusFinishedRoute,
                BusThreadStatus::FinishedUnloadingPassengers,
                BusThreadStatus::Moving,
            ];

            const ALL_MOVING_BUS_VALID_STATUSES: [BusThreadStatus; 2] =
                [BusThreadStatus::BusFinishedRoute, BusThreadStatus::Moving];
            // TODO: add a function for incrementing the timetick from the bus loading phase
            // and use that function instead of this messy refactor
            if current_time_tick.stage == TimeTickStage::BusUnloadingPassengers
                && bus_status_array.iter().all(|bus_thread_status| {
                    UNLOADING_BUS_VALID_STATUSES
                        .iter()
                        .any(|valid_status| bus_thread_status == valid_status)
                })
            {
                println!(
                    "Finished Timestep {:?}. Bus Status Array: {:?}",
                    current_time_tick, bus_status_array
                );
                current_time_tick.increment_time_tick();
                // If all the buses are moving, only one message will be sent to the sync thread for the whole time tick.
                // Increment the time tick twice to make up for that

                if bus_status_array
                    .iter()
                    .all(|bus_thread_status: &BusThreadStatus| {
                        ALL_MOVING_BUS_VALID_STATUSES
                            .iter()
                            .any(|valid_status| bus_thread_status == valid_status)
                    })
                {
                    println!("All buses moving on time step {}", current_time_tick.number);
                    // If all the buses are moving, increase the time tick again, because there will be no stopped bus
                    // to send the next message
                    manage_time_tick_increase_for_finished_loading_tick(
                        current_time_tick,
                        &station_senders,
                        &mut bus_status_array,
                    );
                } else {
                    println!("At least one bus was unloading at this time step");
                }
                continue;

                // advance_and_drop_time_step(current_time_tick, &station_sender);
            } else if let TimeTickStage::BusLoadingPassengers { .. } = current_time_tick.stage {
                if (bus_status_array.iter().all(|bus_thread_status| {
                    LOADING_BUS_VALID_STATUSES
                        .iter()
                        .any(|valid_status| bus_thread_status == valid_status)
                })) {
                    println!(
                        "Finished Timestep {:?}. Bus Status Array: {:?}",
                        current_time_tick, bus_status_array
                    );
                    // This message will only be sent after the bus unloading stage is finished
                    for location_index in 0..GLOBAL_LOCATION_COUNT {
                        station_senders[location_index]
                            .send(SyncToStationMessages::AdvanceTimeStep(*current_time_tick))
                            .unwrap();
                    }
                    manage_time_tick_increase_for_finished_loading_tick(
                        current_time_tick,
                        &station_senders,
                        &mut bus_status_array,
                    );
                    continue;
                }
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

    let passenger_thread_bus_route_clone = bus_route_vec_arc.clone();

    let passenger_thread_passenger_list_clone = passenger_list_pointer.clone();

    let passenger_thread_program_end_clone = program_end.clone();

    let passenger_thread_time_tick_clone = current_time_tick.clone();
    let passenger_thread_sender = tx_from_bus_threads.clone();
    let station_sender_list = send_to_station_channels_arc.clone();
    let passengers_thread_handle = std::thread::spawn(move || {
        let stations_receiver = rx_stations_to_passengers;
        let mut rejected_passengers: Vec<Passenger> = Vec::new();
        let receiver_from_sync_thread = rx_to_passengers;
        let mut previous_time_tick = TimeTick::default();
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

            // if
            //  time_tick.number == 0
            // || time_tick.number % 2 == 0

            // previous_time_tick == *time_tick {
            //     drop(time_tick);
            //     std::thread::sleep(std::time::Duration::from_millis(1));
            //     continue;
            // } else {
            //     previous_time_tick = *time_tick;
            // }
            let bus_route_list = passenger_thread_bus_route_clone.lock().unwrap();

            println!("Bus route list: {bus_route_list:#?}");
            drop(bus_route_list);

            // It may not be neccessary to do this on the first time tick
            if (*time_tick).stage == TimeTickStage::PassengerInit {
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
                    println!("Init Passenger Info Message. Location Index: {index}");
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

    let mut station_handle_list = station::get_station_threads(
        station_location_list.as_ref(),
        &current_time_tick,
        &send_to_bus_channels_arc,
        &receive_in_station_channels_arc,
        &bus_route_vec_arc,
        &passenger_bus_route_arc,
        &rejected_passengers_pointer,
        tx_stations_to_passengers,
        sync_to_stations_receiver,
    );

    handle_list.append(&mut station_handle_list);

    for _ in 0..NUM_OF_BUSES {
        let bus_route_array_clone = bus_route_vec_arc.clone();
        let station_senders_clone = send_to_station_channels_arc.clone();
        let sender = tx_from_bus_threads.clone();
        let bus_receiver_channels = bus_receiver_channels_arc.clone();

        // let passenger_list_pointer_clone = passenger_list_pointer.clone();
        // let passenger_stops_passed_pointer_clone = passenger_extra_stops_waited_pointer.clone();
        let current_time_tick_clone = current_time_tick.clone();
        let mut time_clone_check = 0;
        let handle = std::thread::spawn(move || {
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

            // This works for inital time tick because loop will be skipped for TimeTickStage::PassengerInit anyway
            let mut previous_time_tick = TimeTick {
                number: 0,
                stage: TimeTickStage::PassengerInit,
            };
            loop {
                // println!("Bus {} Loop Beginning", simulated_bus.bus_num);
                let current_time_tick = current_time_tick_clone.lock().unwrap();
                // println!("Bus loop beginning. time tick: {current_time_tick}");

                // Only run each time tick twice. Also the PassengerInit stage does not involve the buses
                if *current_time_tick == previous_time_tick
                    || (*current_time_tick).stage == TimeTickStage::PassengerInit
                {
                    drop(current_time_tick);
                    continue;
                }
                previous_time_tick = *current_time_tick;
                // println!(
                //     // Note, only one bus ever prints this at a time
                //     "Bus {} bus route loop. Time tick: {}",
                //     { simulated_bus.bus_num },
                //     current_time_tick
                // );
                if time_clone_check == (*current_time_tick).number {
                    // println!("Time tick skipped");
                    drop(current_time_tick);
                    continue;
                } else {
                    time_clone_check = (*current_time_tick).number;
                }
                let time_tick = *current_time_tick;

                drop(current_time_tick);

                let bus_update_output = simulated_bus.update(
                    station_senders_clone.as_ref(),
                    &bus_receiver,
                    &sender,
                    &current_time_tick_clone,
                );

                if bus_update_output == ControlFlow::Break(()) {
                    break;
                }
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
