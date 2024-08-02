use crate::bus::Bus;
use crate::consts::{
    DEFAULT_BUS_CAPACITY, DEFAULT_GLOBAL_LOCATION_COUNT, DEFAULT_NUM_OF_BUSES,
    GLOBAL_PASSENGER_COUNT, WRITE_JSON,
};
use crate::data;
use crate::data::InputDataStructure;
use crate::initialize_channel_list;
use crate::location::{BusLocation, PassengerBusLocation};
use crate::station;
use crate::thread::{
    BusMessages, BusThreadStatus, StationMessages, StationToMainMessages,
    StationToPassengersMessages, SyncToStationMessages,
};
use crate::{Location, Passenger};
use crate::{TimeTick, TimeTickStage};

use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::os::unix::process;
use std::sync::{self, mpsc, Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct FinalPassengerLists {
    pub location_lists: Vec<Vec<Passenger>>,
    // Global Location Count
    pub len: usize,
    pub remaining_passengers: Vec<Passenger>,
}

pub struct ConfigStruct {
    pub num_of_buses: usize,
    pub num_of_locations: usize,
    // TODO: turn this into a property on the buses instead of the global config_structure
    pub bus_capacity: u32,
}

// The main bus system loop. This should probably output something eventually to give something to test against
// The return value might be a object containing lists of passengers from every location and the list of passengers that have not
// gotten on any bus
pub fn main_loop(
    location_vector: Vec<Location>,
    total_passenger_list: Vec<Passenger>,
    all_bus_routes: Vec<Vec<BusLocation>>,
    config: ConfigStruct,
) {
    // let passenger_bus_route_list: Vec<Vec<PassengerBusLocation>> = bus_route_array
    //     .clone()
    //     .into_iter()
    //     .map(crate::convert_bus_route_list_to_passenger_bus_route_list)
    //     .collect();

    // let rejected_passengers_pointer = Arc::new(Mutex::new(Vec::<Passenger>::new()));

    let passenger_list_pointer: Arc<Mutex<Vec<Passenger>>> =
        Arc::new(Mutex::new(total_passenger_list));

    let location_vector_arc: Arc<Vec<Location>> = Arc::new(location_vector);

    let bus_route_vec_arc: Arc<Mutex<Vec<Vec<BusLocation>>>> = Arc::new(Mutex::new(all_bus_routes));

    // let passenger_bus_route_arc: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>> =
    //     Arc::new(Mutex::new(passenger_bus_route_list));

    // let passenger_extra_stops_waited_pointer: Arc<Mutex<Vec<u32>>> =
    //     Arc::new(Mutex::new(Vec::<u32>::new()));
    // let final_passengers_arc = Arc::new(Mutex::new(FinalPassengerLists::default()));

    // // Split time ticks into two - time ticks are accurate
    // let current_time_tick: Arc<Mutex<TimeTick>> = Arc::new(Mutex::new(TimeTick::default()));

    let program_end: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    // let mut handle_list: Vec<std::thread::JoinHandle<()>> = vec![];

    let sync_handle_program_end_clone: Arc<Mutex<bool>> = program_end.clone();

    let (tx_from_bus_threads, rx_from_threads) = mpsc::channel::<BusMessages>();

    let (tx_to_passengers, rx_to_passengers) = mpsc::channel::<Option<Vec<Passenger>>>();

    // let (tx_stations_to_passengers, mut rx_stations_to_passengers) =
    //     mpsc::channel::<StationToPassengersMessages>();
    // let (send_to_station_channels, receive_in_station_channels) =
    //     crate::initialize_channel_list::<StationMessages>(GLOBAL_LOCATION_COUNT);
    // let receive_in_station_channels: Vec<Option<_>> =
    //     receive_in_station_channels.into_iter().map(Some).collect();

    // let send_to_station_channels_arc = Arc::new(send_to_station_channels);
    // let receive_in_station_channels_arc = Arc::new(Mutex::new(receive_in_station_channels));

    // let (send_to_bus_channels, receive_from_bus_channels) =
    initialize_channel_list::<crate::thread::StationToBusMessages>(DEFAULT_NUM_OF_BUSES);

    // let bus_receiver_channels_arc = Arc::new(Mutex::new(receive_in_bus_channels));
    // let send_to_bus_channels_arc = Arc::new(send_to_bus_channels);

    // let current_time_tick_clone = current_time_tick.clone();

    // #[track_caller]
    // fn increment_and_drop_time_step(
    //     mut time_tick: sync::MutexGuard<'_, TimeTick>,
    //     station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
    // ) {
    //     let call_location = std::panic::Location::caller();
    //     println!("---------- All buses are finished at their stops -----------");
    //     println!("Time step incremented from {}", call_location);
    //     // At this point sending the message here is not neccesary.
    //     // Because the time step mutiex is held for both time step increments,
    //     // the station will be stuck in the unloading phase when all the buses are moving
    //     // Eventually a message might be neccesary for that case but this implimentation works for now
    //     /*
    //       // This message will only be sent after the bus unloading stage is finished
    //         station_sender
    //           .send(SyncToStationMessages::AdvanceTimeStep(*time_tick))
    //           .unwrap();
    //     */
    //     (*time_tick).increment_time_tick();
    // }

    // fn manage_time_tick_increase_for_finished_loading_tick(
    //     mut current_time_tick: sync::MutexGuard<'_, TimeTick>,
    //     station_senders: &Vec<mpsc::Sender<SyncToStationMessages>>,
    //     bus_status_array: &mut [BusThreadStatus; NUM_OF_BUSES],
    // ) {
    //     println!("Bus statuses: {:?}", &bus_status_array);
    //     // The bus_loading timestep is finished, so the array is reset the entire status array
    //     for status in bus_status_array.iter_mut() {
    //         // Reset the statuses for the next time step
    //         if status == &BusThreadStatus::FinishedLoadingPassengers
    //             || status == &BusThreadStatus::Moving
    //         {
    //             *status = BusThreadStatus::WaitingForTimeStep;
    //         }
    //     }
    //     increment_and_drop_time_step(current_time_tick, station_senders);

    //     println!("End of sync loop");
    // }

    let (sender_sync_to_stations_list, receiver_sync_to_stations_list) =
        initialize_channel_list::<SyncToStationMessages>(DEFAULT_GLOBAL_LOCATION_COUNT);

    let (sender_stations_to_sync_list, receiver_sync_from_stations_list) =
        initialize_channel_list::<StationToMainMessages>(DEFAULT_GLOBAL_LOCATION_COUNT);

    let receiver_sync_to_stations_list: Vec<_> = receiver_sync_to_stations_list
        .into_iter()
        .map(Some)
        .collect();
    let sync_to_stations_receiver = Arc::new(Mutex::new(receiver_sync_to_stations_list));

    // spawn other threads
    let main_handle = std::thread::spawn(|| {});

    let send_to_stations = sender_sync_to_stations_list.clone();
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
    let current_time_tick = TimeTick {
        number: 0,
        stage: TimeTickStage::PassengerInit,
    };

    let route_sync_location_vec_arc = location_vector_arc.clone();
    let route_sync_passenger_list_arc = passenger_list_pointer.clone();
    let route_sync_bus_route_vec_arc = bus_route_vec_arc.clone();

    loop {
        // bus index could be helpful for

        // Receive and process game-terminating messages from the buses
        let message_from_buses = receiver_from_buses.try_recv().unwrap();
        // Receive panics from all
        if let crate::thread::BusMessages::BusPanicked {
            bus_index: _,
            ref message,
        } = message_from_buses
        {
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

        for bus_receiver in receiver_sync_from_stations_list.iter() {
            let incoming_message = bus_receiver.receiver.try_recv().unwrap();
            // Convert to if then statement when ther
            let crate::thread::StationToMainMessages::CrashProgram { ref message } =
                incoming_message;
            for station_sender in send_to_stations.iter() {
                station_sender
                    .send(SyncToStationMessages::ProgramFinished(
                        crate::thread::ProgramEndType::ProgramCrashed {
                            message: message.to_string(),
                        },
                    ))
                    .unwrap();
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

                continue;
            }

            println!(
                "There were rejected passengers received. Count: {}",
                rejected_passengers_list.len()
            );

            passenger_sender
                .send(Some(rejected_passengers_list.clone()))
                .unwrap();

            rejected_passengers_list.clear();
        }

        let received_bus_stop_message = receiver_from_buses.recv().unwrap();

        match received_bus_stop_message {
            BusMessages::BusPanicked { .. } => {
                panic!("{received_bus_stop_message:?} should have been processed already");
            }

            BusMessages::AdvanceTimeStepForUnloadedBus { bus_index } => {
                bus_status_vector[bus_index] = BusThreadStatus::FinishedUnloadingPassengers;
            }

            BusMessages::AdvanceTimeStepForLoadedBus { bus_index } => {
                bus_status_vector[bus_index] = BusThreadStatus::FinishedLoadingPassengers;
            }

            BusMessages::AdvanceTimeStepForMovingBus { bus_index } => {
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
                println!("Passenger initialized");
                // TODO: increment time step
            }
        }

        println!("Processed received: {processed_bus_received_count}");
        println!(
            "{}",
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

            println!(
                "All Buses Initialized. Time tick 0 message: {:?}",
                received_bus_stop_message
            );
            continue;
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

            println!("Program Complete");
            break;
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

            // TODO: send the increase time tick message

            if bus_status_vector.iter().all(|bus_thread_status| {
                ALL_MOVING_BUS_VALID_STATUSES
                    .iter()
                    .any(|valid_status| bus_thread_status == valid_status)
            }) {
                println!("All buses moving on time step {}", current_time_tick.number);

                // TODO: increment the time tick
            } else {
                println!(
                    "Finished timestep {:?}. Bus Status Vector: {:?}",
                    current_time_tick, bus_status_vector
                );
            }
            continue;
        } else if let TimeTickStage::BusLoadingPassengers { .. } = current_time_tick.stage {
            if (bus_status_vector.iter().all(|bus_thread_status| {
                LOADING_BUS_VALID_STATUSES
                    .iter()
                    .any(|valid_status| bus_thread_status == valid_status)
            })) {
                println!(
                    "Finished Timestep {:?}. Bus Status Array: {:?}",
                    current_time_tick, bus_status_vector
                );

                for station_sender in &send_to_stations {
                    station_sender
                        .send(SyncToStationMessages::AdvanceTimeStep(current_time_tick))
                        .unwrap();
                }
            }

            // TODO: Increment time tick

            continue;
        }
    }

    {}
}
