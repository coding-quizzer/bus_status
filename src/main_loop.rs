use crate::bus::Bus;
use crate::consts::WRITE_JSON;
use crate::data;
use crate::data::InputDataStructure;
use crate::initialize_channel_list;
use crate::location::{BusLocation, PassengerBusLocation};
use crate::station;
use crate::thread::{
    BusMessages, BusThreadStatus, StationEventMessages, StationToPassengersMessages,
    StationToSyncMessages, SyncToBusMessages, SyncToStationMessages,
};
use crate::{Location, Passenger};
use crate::{TimeTick, TimeTickStage};

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::mpsc::TryRecvError;
use std::sync::{mpsc, Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct FinalPassengerLists {
    pub location_lists: Vec<Vec<Passenger>>,
    // Global Location Count
    pub len: usize,
    pub remaining_passengers: Vec<Passenger>,
}

#[derive(Clone, Copy)]
pub struct ConfigStruct {
    pub num_of_buses: usize,
    pub num_of_passengers: usize,
    pub num_of_locations: usize,
    // TODO: turn this into a property on the buses instead of the global config_structure
    pub bus_capacity: u32,
}

// The main bus system loop. This should probably output something eventually to give something to test against
// The return value might be a object containing lists of passengers from every location and the list of passengers that have not
// gotten on any bus
//
// Tne distance of the locations must be a multiple of 2 because there are 2 time tick stages
pub fn run_simulation(
    location_vector: Vec<Location>,
    total_passenger_list: Vec<Passenger>,
    all_bus_routes: Vec<Vec<BusLocation>>,
    config: ConfigStruct,
) -> FinalPassengerLists {
    let passenger_bus_route_list: Vec<Vec<PassengerBusLocation>> = all_bus_routes
        .clone()
        .into_iter()
        .map(crate::convert_bus_route_list_to_passenger_bus_route_list)
        .collect();

    let rejected_passengers_pointer = Arc::new(Mutex::new(Vec::<Passenger>::new()));

    let passenger_list_pointer: Arc<Mutex<Vec<Passenger>>> =
        Arc::new(Mutex::new(total_passenger_list));

    let location_vector_arc: Arc<Vec<Location>> = Arc::new(location_vector);

    let bus_route_vec_arc: Arc<Mutex<Vec<Vec<BusLocation>>>> = Arc::new(Mutex::new(all_bus_routes));

    let passenger_bus_route_arc: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>> =
        Arc::new(Mutex::new(passenger_bus_route_list));

    // let passenger_extra_stops_waited_pointer: Arc<Mutex<Vec<u32>>> =
    //     Arc::new(Mutex::new(Vec::<u32>::new()));
    let final_passengers_arc = Arc::new(Mutex::new(FinalPassengerLists::default()));

    // // Split time ticks into two - time ticks are accurate
    // let current_time_tick: Arc<Mutex<TimeTick>> = Arc::new(Mutex::new(TimeTick::default()));

    let program_end: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let mut handle_list: Vec<std::thread::JoinHandle<()>> = vec![];

    let sync_handle_program_end_clone: Arc<Mutex<bool>> = program_end.clone();

    let (tx_from_bus_threads, rx_from_threads) = mpsc::channel::<BusMessages>();

    let (tx_to_passengers, rx_to_passengers) = mpsc::channel::<Option<Vec<Passenger>>>();

    let (tx_stations_to_passengers, rx_stations_to_passengers) =
        mpsc::channel::<StationToPassengersMessages>();
    let (send_to_station_channels, receive_in_station_channels) =
        crate::initialize_channel_list::<StationEventMessages>(config.num_of_locations);
    let receive_in_station_channels: Vec<Option<_>> =
        receive_in_station_channels.into_iter().map(Some).collect();

    let send_to_station_channels_arc = Arc::new(send_to_station_channels);

    let (send_to_bus_channels, receive_from_bus_channels) =
        initialize_channel_list::<crate::thread::StationToBusMessages>(config.num_of_buses);

    // let current_time_tick_clone = current_time_tick.clone();

    #[track_caller]
    fn increment_time_step(
        is_first_run: bool,
        all_buses_are_moving: bool,
        time_tick: &mut TimeTick,
        station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
        bus_senders: &Vec<mpsc::Sender<SyncToBusMessages>>,
        bus_status_vector: &mut [BusThreadStatus],
    ) {
        let call_location = std::panic::Location::caller();
        println!("---------- All buses finished current time step -----------");
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
        for status in bus_status_vector.iter_mut() {
            // Reset the statuses for the next time step
            if status != &BusThreadStatus::BusFinishedRoute {
                *status = BusThreadStatus::WaitingForTimeStep;
            }
        }

        if is_first_run {
            time_tick.increment_from_initialized();
        } else {
            time_tick.increment_time_tick();
        }
        for station_sender in station_senders {
            station_sender
                .send(SyncToStationMessages::AdvanceTimeStep(*time_tick))
                .unwrap();
        }

        for bus_sender in bus_senders {
            bus_sender
                .send(SyncToBusMessages::AdvanceTimeStep(*time_tick))
                .unwrap_or(());
        }
    }

    // fn manage_time_tick_increase_for_finished_loading_tick(
    //     mut current_time_tick: sync::MutexGuard<'_, TimeTick>,
    //     station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
    //     bus_status_array: &mut [BusThreadStatus; NUM_OF_BUSES],
    // ) {
    //     println!("Bus statuses: {:?}", &bus_status_array);
    //     // The bus_loading timestep is finished, so the array is reset the entire status array

    //     increment_and_drop_time_step(current_time_tick, station_senders);

    //     println!("End of sync loop");

    // }

    let (sender_sync_to_stations_list, receiver_sync_to_stations_list) =
        initialize_channel_list::<SyncToStationMessages>(config.num_of_locations);
    println!("channel count: {}", config.num_of_locations);
    println!(
        "Sync to stations list: {:?}",
        receiver_sync_to_stations_list
    );

    let (sender_sync_to_bus_list, receiver_sync_to_bus_list) =
        initialize_channel_list::<SyncToBusMessages>(config.num_of_buses);
    let receiver_sync_to_bus_channels = receiver_sync_to_bus_list
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();

    let (sender_stations_to_sync_list, receiver_sync_from_stations_list) =
        initialize_channel_list::<StationToSyncMessages>(config.num_of_locations);

    let receiver_sync_to_stations_list: Vec<_> = receiver_sync_to_stations_list
        .into_iter()
        .map(Some)
        .collect();
    let sync_to_stations_receiver = Arc::new(Mutex::new(receiver_sync_to_stations_list));

    let send_to_bus_channels_arc = Arc::new(send_to_bus_channels);
    let receive_in_station_channels_arc = Arc::new(Mutex::new(receive_in_station_channels));

    // TODO: Change the station_handle_list function to deal with the time tick
    let station_location_list = location_vector_arc.clone();

    let station_time_tick = TimeTick::default();

    let mut station_handle_list = station::get_station_threads(
        &station_location_list,
        &station_time_tick,
        &send_to_bus_channels_arc,
        &receive_in_station_channels_arc,
        &bus_route_vec_arc,
        &passenger_bus_route_arc,
        &rejected_passengers_pointer,
        tx_stations_to_passengers,
        sync_to_stations_receiver,
        &final_passengers_arc,
        &config,
    );

    handle_list.append(&mut station_handle_list);

    // Beginning of bus thread loops

    let bus_receiver_channels_arc = Arc::new(Mutex::new(receive_from_bus_channels));
    let receiver_sync_to_bus_channels_arc = Arc::new(Mutex::new(receiver_sync_to_bus_channels));

    for _ in 0..config.num_of_buses {
        let receiver_sync_to_bus_channels_clone = receiver_sync_to_bus_channels_arc.clone();
        let bus_route_vector_clone = bus_route_vec_arc.clone();
        let station_senders_clone = send_to_station_channels_arc.clone();
        let sender = tx_from_bus_threads.clone();
        let bus_receiver_channels_from_station = bus_receiver_channels_arc.clone();

        // let new_receiver_sync_to_bus_list: &mut Vec<
        //     Option<crate::ReceiverWithIndex<SyncToBusMessages>>,
        // > = receiver_sync_to_bus_list.as_mut();
        let current_time_tick = TimeTick::default();

        // TODO: Remove if not useful
        let mut time_clone_check = 0;
        let bus_handle = std::thread::spawn(move || {
            let current_bus_receiver_from_station_with_index =
                bus_receiver_channels_from_station.lock().unwrap().remove(0);

            drop(bus_receiver_channels_from_station);
            // Since the receiver obtained is not deterministic, set the bus index based on what channel was obtained
            let bus_index = current_bus_receiver_from_station_with_index.index;
            let current_bus_receiver_from_sync = receiver_sync_to_bus_channels_clone
                .lock()
                .unwrap()
                .get_mut(bus_index)
                .expect("List should be long enough for all the buses")
                .take()
                .expect("This value should not have been taken already");
            println!("Bus index: {}", bus_index);
            let bus_receiver_from_station = current_bus_receiver_from_station_with_index.receiver;

            let mut bus_route_vector = bus_route_vector_clone.lock().unwrap();
            let bus_route = bus_route_vector.get(bus_index).unwrap();
            println!("Bus {bus_index} bus route: {bus_route:#?}");
            let simulated_bus_option =
                Bus::try_new(bus_route.clone(), config.bus_capacity as usize, bus_index);

            println!(
                "Simulated bus option for bus {}: {:?}",
                bus_index, simulated_bus_option
            );
            // TODO: Convert expect to a message to the sync thread
            let mut simulated_bus = simulated_bus_option
                .expect("Each bus must have more than one location in its route");
            bus_route_vector[simulated_bus.bus_index] = simulated_bus.get_bus_route();
            // Release the lock on bus_route_aray by dropping it
            drop(bus_route_vector);
            sender
                .send(BusMessages::InitBus {
                    bus_index: simulated_bus.bus_index,
                })
                .unwrap();
            println!("Bus Message sent from bus {}", simulated_bus.bus_index);

            let mut previous_time_tick = TimeTick::default();
            // let incoming_message = current_bus_receiver_from_sync.receiver.recv().unwrap();
            // let time_tick: TimeTick = match incoming_message {
            //     SyncToBusMessages::AdvanceTimeStep(time_step) => {
            //         if time_step.number - previous_time_tick.number > 1 {
            //             panic!("Skipped from time tick {previous_time_tick:?} to {time_step:?} ")
            //         } else {
            //             time_step
            //         }
            //     }
            //     // So far, there are no other options
            //     _ => unreachable!(),
            // };
            // println!("Bus thread Time tick incremented. Time tick: {time_tick:?}");
            loop {
                println!("Beginning of Bus loop. Bus num: {}", bus_index);
                // TODO: Set current time tick to the appropriate value, incorporating messages from the main/sync thread
                let incoming_message = current_bus_receiver_from_sync.receiver.recv().unwrap();
                let time_tick: TimeTick = match incoming_message {
                    SyncToBusMessages::AdvanceTimeStep(time_step) => {
                        if time_step.number - previous_time_tick.number > 1 {
                            panic!(
                                "Skipped from time tick {previous_time_tick:?} to {time_step:?} "
                            )
                        } else {
                            time_step
                        }
                    }
                    // So far, there are no other options
                    _ => unreachable!(),
                };
                println!("Bus thread Time tick incremented. Time tick: {time_tick:?}");

                if time_tick == previous_time_tick
                    || (time_tick).stage == TimeTickStage::PassengerInit
                {
                    continue;
                }
                previous_time_tick = time_tick;
                simulated_bus.time_tick = time_tick;

                let bus_update_output = simulated_bus.update(
                    &station_senders_clone,
                    &bus_receiver_from_station,
                    &sender,
                    &current_bus_receiver_from_sync.receiver,
                );

                if bus_update_output == ControlFlow::Break(()) {
                    break;
                }
            }
        });
        handle_list.push(bus_handle);
    }

    /* Beginning of passenger thread loop */

    let passenger_thread_bus_route_list_clone = bus_route_vec_arc.clone();
    let passenger_thead_passenger_list_clone = passenger_list_pointer.clone();
    let passenger_thread_program_end_clone = program_end.clone();
    let mut rejected_passengers: Vec<Passenger> = Vec::new();

    let passenger_thread_time_tick = TimeTick::default();

    // NOTE: how does this work? Why does passenger use the sender that officially should be sent from the bus threads?
    // How can I make the funciton clearer
    let passenger_thread_sender = tx_from_bus_threads.clone();
    let station_sender_list = send_to_station_channels_arc.clone();
    // At this point this pretty much only initializes the passengers to their stations, so it only needs to run once.
    // Because it will send a message that will ultimately be broadcasted to change the timetick. It is guaranteed that the
    // time tick will be the first time tick
    let passengers_thread_handle = std::thread::spawn(move || {
        let stations_receiver = rx_stations_to_passengers;

        // Replace with message sent from main/sync thread
        /*
         if *passenger_thread_program_end_clone.lock().unwrap() {
           println!("Rejected Passenger count: {}", rejected_passengers.len());
           break;
         }
        */

        let bus_route_list = passenger_thread_bus_route_list_clone.lock().unwrap();

        // println!("Bus route list: {bus_route_list:#?}");
        drop(bus_route_list);

        println!("First time tick loop");
        let passenger_list = passenger_thead_passenger_list_clone.lock().unwrap();
        println!("Beginning of tick one passenger calculations");

        let mut passenger_location_list: Vec<Vec<Passenger>> = Vec::new();
        for _ in 0..(config.num_of_locations) {
            passenger_location_list.push(Vec::new());
        }

        let mut location_vec_deque = VecDeque::from(passenger_list.clone());

        while let Some(passenger) = location_vec_deque.pop_back() {
            let current_location_index = passenger.current_location.unwrap().index;
            passenger_location_list[current_location_index].push(passenger);
        }

        for (index, passengers_in_locations) in passenger_location_list.into_iter().enumerate() {
            station_sender_list.as_ref()[index]
                .send(StationEventMessages::InitPassengerList(
                    passengers_in_locations,
                ))
                .unwrap();
            println!("Init Passenger Info Message. Location Index: {index}");
        }

        for _ in 0..config.num_of_locations {
            let sync_message = stations_receiver.recv().unwrap();
            if let StationToPassengersMessages::ConfirmInitPassengerList(station_number) =
                sync_message
            {
                println!("Passenger Init Confirmed from Station {station_number}");
            }
        }

        println!("End of Tick one passenger calculations");

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
        // TODO: Replace with something comperable to the panic message
        assert_eq!(
            passenger_list.len() + rejected_passengers.len(),
            config.num_of_passengers
        );

        // If I decide to use these, I will need to figure out how to

        // let total_rejected_passengers = rejected_passengers.len();
        // println!("There were a total of {total_rejected_passengers} rejected passengers");
    });

    handle_list.push(passengers_thread_handle);

    /* Beginning of Main/sync thread loop */
    // spawn other threads

    let send_to_stations = sender_sync_to_stations_list.clone();
    let send_to_buses = sender_sync_to_bus_list.clone();
    let receiver_from_buses = rx_from_threads;

    // Main thread: Keeps track of the remaining threads as a whole. Sends messages

    // Initialize bus status vector
    let mut bus_status_vector = Vec::new();
    bus_status_vector.resize(config.num_of_buses, BusThreadStatus::Uninitialized);

    let mut processed_bus_received_count = 0;
    let mut processed_moving_bus_received_count;
    let passenger_sender = tx_to_passengers;

    let mut rejected_passengers_list = Vec::new();
    // the time tick is stored here and sent to the other threads when it changes
    let mut current_time_tick = TimeTick {
        number: 0,
        stage: TimeTickStage::PassengerInit,
    };

    let route_sync_location_vec_arc = location_vector_arc.clone();
    let route_sync_passenger_list_arc = passenger_list_pointer.clone();
    let route_sync_bus_route_vec_arc = bus_route_vec_arc.clone();
    let mut passengers_initialized = false;

    'sync_loop: loop {
        println!("Beginning of route sync loop");
        // bus index could be helpful for

        // Receive and process program-terminating messages from the buses

        // Definately something really wrong

        // Timing problem
        /*
        let message_from_buses_result = receiver_from_buses.try_recv();

        match message_from_buses_result {
            Ok(message_from_buses) => {
                println!(
                    "Route sync message received. Message: {:?}",
                    message_from_buses
                );
                // Receive panics from all
                if let crate::thread::BusMessages::BusPanicked {
                    bus_index: _,
                    ref message,
                } = message_from_buses
                {

                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                panic!("{}", TryRecvError::Disconnected)
            }
        };
        */

        // Remove once confirmed bus finished case is dealt with already. Remove SyncToStationMessages::ProgramFinished as well
        /* else if let crate::thread::BusMessages::BusFinished { bus_index: _ } = message_from_buses
        {
            // this is not completely right. I need to set up the situation where this is really accurate
            for station_sender in send_to_stations.iter() {
                station_sender
                    .send(SyncToStationMessages::ProgramFinished(
                        crate::thread::ProgramEndType::ProgramFinished,
                    ))
                    .unwrap();
            }
        } */

        // Receive any crash signal from threads and send it to all the threads
        for sync_receiver in receiver_sync_from_stations_list.iter() {
            let incoming_message = sync_receiver.receiver.try_recv();
            let incoming_message = match incoming_message {
                Ok(message) => {
                    println!("Sync message received from station: {message:?}");
                    message
                }
                Err(TryRecvError::Empty) => {
                    // Weirdly, this prints exactly 5 times and then stops
                    println!("No message received. Continuing");
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("{}", TryRecvError::Disconnected)
                }
            };
            // Convert to if then statement if there are more kinds of messages
            if let crate::thread::StationToSyncMessages::CrashProgram { ref message } =
                incoming_message
            {
                for station_sender in send_to_stations.iter() {
                    station_sender
                        .send(SyncToStationMessages::ProgramFinished(
                            crate::thread::ProgramEndType::ProgramCrashed {
                                message: message.into(),
                            },
                        ))
                        .unwrap();
                }
            }
        }

        if processed_bus_received_count
            == bus_status_vector
                .iter()
                .filter(|status| *status != &BusThreadStatus::BusFinishedRoute)
                .count()
        {
            // Reset the processed_bus_received count for the next cycle
            processed_bus_received_count = 0;
            processed_moving_bus_received_count = 0;
            if rejected_passengers_list.is_empty() {
                passenger_sender.send(None).unwrap();
            } else {
                println!(
                    "There were rejected passengers received. Count: {}",
                    rejected_passengers_list.len()
                );

                passenger_sender
                    .send(Some(rejected_passengers_list.clone()))
                    .unwrap();

                rejected_passengers_list.clear();
            }
        }
        println!("Waiting for message from bus");
        // At this point, the program is held up here
        let received_bus_stop_message = receiver_from_buses.recv().unwrap();
        println!("Bus message received: {received_bus_stop_message:?}. ");

        match received_bus_stop_message {
            BusMessages::BusPanicked {
                bus_index: _,
                ref message,
            } => {
                for station_sender in &send_to_stations {
                    station_sender
                        .send(SyncToStationMessages::ProgramFinished(
                            crate::thread::ProgramEndType::ProgramCrashed {
                                message: message.to_string(),
                            },
                        ))
                        .unwrap();
                }
            }

            BusMessages::AdvanceTimeStepForUnloadedBus { bus_index } => {
                bus_status_vector[bus_index] = BusThreadStatus::FinishedUnloadingPassengers;
            }

            BusMessages::AdvanceTimeStepForLoadedBus { bus_index } => {
                // DEBUG: why did all the buses send this message, if bus 2 did not exit the station
                println!("Bus {bus_index} is finished loading.");

                bus_status_vector[bus_index] = BusThreadStatus::FinishedLoadingPassengers;
            }

            BusMessages::AdvanceTimeStepForMovingBus { bus_index } => {
                println!(
                    "Sync thread received message from Bus {} for moving bus",
                    bus_index
                );
                bus_status_vector[bus_index] = BusThreadStatus::Moving
            }

            BusMessages::BusFinished { bus_index } => {
                bus_status_vector[bus_index] = BusThreadStatus::BusFinishedRoute;
                println!("Bus {} Finished Route", bus_index);
                let finished_bus_count = bus_status_vector
                    .iter()
                    .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
                    .count();
                let active_bus_count = config.num_of_buses - finished_bus_count;
                println!("There are now {active_bus_count} active buses.");
            }

            BusMessages::InitBus { bus_index } => {
                bus_status_vector[bus_index] = BusThreadStatus::WaitingForTimeStep;
                println!("Bus {bus_index} Initialized");
            }

            BusMessages::InitPassengers => {
                if (current_time_tick.number == 0) {
                    println!("Passengers initialized");
                    passengers_initialized = true;
                }
                // Are passengers set up to initialize after the buses
                println!("Passengers initialized");
            }
        }

        println!("Processed received: {processed_bus_received_count}");
        println!("Bus Status Vector: {:?}", bus_status_vector);
        println!(
            "Number of buses not finished: {}",
            bus_status_vector
                .iter()
                .filter(|status| *status != &BusThreadStatus::BusFinishedRoute)
                .count()
        );

        if current_time_tick.stage == TimeTickStage::PassengerInit
            && bus_status_vector
                .iter()
                .filter(|status| *status == &BusThreadStatus::Uninitialized)
                .count()
                == 0
        {
            // time tick is not dropped anymore, because this thread owns the timetick

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

            if passengers_initialized {
                increment_time_step(
                    true,
                    false,
                    &mut current_time_tick,
                    &send_to_stations,
                    &send_to_buses,
                    &mut bus_status_vector,
                );
            }

            println!(
                "All Buses Initialized. Time tick 0 message: {:?}",
                received_bus_stop_message
            );

            // Time tick could increment here
        }

        let finished_buses = bus_status_vector
            .iter()
            .filter(|status| *status == &BusThreadStatus::BusFinishedRoute)
            .count();

        println!("There are {finished_buses} finished buses.");

        if finished_buses >= config.num_of_buses {
            // TODO: convert this to a message sent from main/sync thread
            let mut program_end = sync_handle_program_end_clone.lock().unwrap();
            *program_end = true;
            for sender in send_to_stations {
                sender
                    .send(SyncToStationMessages::ProgramFinished(
                        crate::thread::ProgramEndType::ProgramFinished,
                    ))
                    .unwrap();
            }

            // This is the place for code after the loop

            println!("Program Complete");
            break 'sync_loop;
        }

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

        // if bus_status_vector
        //     .iter()
        //     .all(|bus_status| bus_status == BusThreadStatus::BusFinishedRoute)
        // {
        //     println!("")
        //     break;
        // }

        if current_time_tick.stage == TimeTickStage::BusUnloadingPassengers
            && bus_status_vector.iter().all(|bus_thread_status| {
                UNLOADING_BUS_VALID_STATUSES
                    .iter()
                    .any(|valid_status| bus_thread_status == valid_status)
            })
        {
            println!(
                "Finished Timestep {:?}. Bus Status Vector: {:?}",
                current_time_tick, bus_status_vector
            );

            let all_buses_are_moving = bus_status_vector.iter().all(|bus_thread_status| {
                ALL_MOVING_BUS_VALID_STATUSES
                    .iter()
                    .any(|valid_status| bus_thread_status == valid_status)
            });

            // TODO: increment the time tick
            // Increment the time-step by one if the buses are moving and by 2 if they are not moving
            increment_time_step(
                false,
                all_buses_are_moving,
                &mut current_time_tick,
                &send_to_stations,
                &send_to_buses,
                &mut bus_status_vector,
            );
        } else if let TimeTickStage::BusLoadingPassengers { .. } = current_time_tick.stage {
            if (bus_status_vector.iter().all(|bus_thread_status| {
                LOADING_BUS_VALID_STATUSES
                    .iter()
                    .any(|valid_status| bus_thread_status == valid_status)
            })) {
                println!(
                    "Finished Timestep {:?}. Bus Status Vector: {:?}",
                    current_time_tick, bus_status_vector
                );

                increment_time_step(
                    false,
                    false,
                    &mut current_time_tick,
                    &send_to_stations,
                    &send_to_buses,
                    &mut bus_status_vector,
                );
            }
        }
        println!(
            "Bus Route Vector at end of Main Loop: {:?}",
            bus_status_vector
        );

        println!("End of route sync loop. Line 713.");
        //  println!("Time tick: {:?}", current_time_tick);
    }

    for handle in handle_list {
        println!("Joined handle {:?}", handle.thread().id());
        handle.join().unwrap();
    }

    let rejected_passengers = rejected_passengers_pointer.lock().unwrap();

    println!(
        "Number of rejected passengers: {}",
        rejected_passengers.len()
    );

    println!("Rejected Passengers: {:#?}", rejected_passengers);
    return final_passengers_arc.lock().unwrap().clone();
}
