use crate::bus::{BusLocation, SendableBus};
use crate::location::{Location, PassengerBusLocation};
use crate::passenger::Passenger;
use crate::passenger::PassengerOnboardingBusSchedule;
use crate::{
    calculate_passenger_schedule_for_bus,
    calculate_passenger_schedule_for_bus_check_available_buses,
};
use crate::thread::{StationMessages, StationToBusMessages, StationToPassengersMessages};
use std::os::linux::raw::stat;
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::thread::{self, JoinHandle};
use crate::consts::NUM_OF_BUSES;
use crate::ReceiverWithIndex;

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

use std::collections::VecDeque;

#[derive(Debug)]
pub struct Station {
    pub location: Location,
    pub docked_buses: Vec<SendableBus>,
    pub passengers: Vec<Passenger>,
    pub buses_unavailable: Vec<usize>,
}

impl Station {
    pub fn new(location: Location) -> Self {
        Station {
            location,
            docked_buses: Vec::new(),
            passengers: Vec::new(),
            buses_unavailable: Vec::new(),
        }
    }

    // TODO: Convert to a function returning an Option, figure out
    // I might have done it - clarify
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
            calculate_passenger_schedule_for_bus(&new_passenger, time_tick, bus_route_list)?;
        println!("New Bus Schedule: {:#?}", new_bus_schedule);
        println!("Passengers added.");
        new_passenger.bus_schedule_iterator = new_bus_schedule.clone().into_iter().peekable();
        new_passenger.bus_schedule = new_bus_schedule;
        self.passengers.push(new_passenger);
        Ok(())
    }

    pub fn add_passenger_check_available_buses(
        &mut self,
        mut new_passenger: Passenger,
        current_time_tick: u32,
        bus_route_list: &Vec<Vec<PassengerBusLocation>>,
        buses_unavailable: Vec<usize>,
    ) -> Result<(), Passenger> {
        // TODO: Add tests to prove that this method of removing those indeces actually work
        println!(
            "Calculating Route from {} to {}.",
            new_passenger.current_location.unwrap(),
            new_passenger.destination_location
        );

        let new_bus_schedule = calculate_passenger_schedule_for_bus_check_available_buses(
            &new_passenger,
            current_time_tick,
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

/// 
/// 
pub fn get_station_threads (station_location_list: &Vec<Location>,  current_time_tick: &Arc<Mutex<u32>>, send_to_bus_channels_arc: &Arc<Vec<Sender<StationToBusMessages>>>, receive_in_station_channels_arc: &Arc<Mutex<Vec<ReceiverWithIndex<StationMessages>>>>, bus_route_vec_arc: &Arc<Mutex<[Vec<BusLocation>; NUM_OF_BUSES]>>, passenger_bus_route_arc: &Arc<Mutex<Vec<Vec<PassengerBusLocation>>>>, rejected_passengers_pointer: &Arc<Mutex<Vec<Passenger>>>, tx_stations_to_passengers: Sender<StationToPassengersMessages>) -> Vec<JoinHandle<()>> {
    // let station_location_list = location_vector_arc.clone();

  let mut handle_list = Vec::new();
  
  // Station index and location index are equivilant
  for location in station_location_list {
    let station_time_tick = current_time_tick.clone();
    let current_location = *location;
    let send_to_bus_channels = send_to_bus_channels_arc.clone();
    let station_channels = receive_in_station_channels_arc.clone();
    let bus_route_list = bus_route_vec_arc.clone();
    let station_thread_passenger_bus_route_list = passenger_bus_route_arc.clone();
    // let send_to_bus_channels =
    let rejected_passenger_clone = rejected_passengers_pointer.clone();
    
    let to_passengers_sender_clone = tx_stations_to_passengers.clone();
    let station_handle = create_station_thread(current_location,  station_time_tick, send_to_bus_channels, station_channels, bus_route_list, station_thread_passenger_bus_route_list, rejected_passenger_clone, to_passengers_sender_clone);
    handle_list.push(station_handle);
  }

  handle_list
    

}

  pub fn create_station_thread (current_location: Location,  station_time_tick: Arc<Mutex<u32>>, send_to_bus_channels: Arc<Vec<Sender<StationToBusMessages>>>, station_channels: Arc<Mutex<Vec<ReceiverWithIndex<StationMessages>>>>, bus_route_list: Arc<Mutex<[Vec<BusLocation>; NUM_OF_BUSES]>>, station_thread_passenger_bus_route_list: Arc<Mutex<Vec<Vec<PassengerBusLocation>>>>, rejected_passenger_clone: Arc<Mutex<Vec<Passenger>>>, to_passengers_sender_clone: Sender<StationToPassengersMessages>) -> JoinHandle<()> {
    let station_handle = thread::spawn(move || {
            let mut current_station = Station::new(current_location);
            let mut previous_time_tick = 0;
            let current_receiver_with_index = station_channels.lock().unwrap().remove(0);
            let station_index = current_receiver_with_index.index;
            println!("Station index: {station_index}");
            let current_receiver = current_receiver_with_index.receiver;
            
            
            loop {
              // println!("Station {} loop beginning", station_index);
                let current_time_tick = station_time_tick.lock().unwrap();
                // println!(
                //     "Station {} loop beginning. Time tick: {}",
                //     station_index, time_tick
                // );
                // println!("Station {location_index}. time tick: {}", *time_tick);
                
                // When time tick is 0, previous_time_tick is 0, so the loop is skipped. This is fine, since the station
                // does not do anything on time tick 0.
                if previous_time_tick != *current_time_tick {
                  previous_time_tick = *current_time_tick;
                } else {
                    drop(current_time_tick);
                    continue;
                  }

                  if *current_time_tick == 1 {
                    println!("First time tick station {} thread", station_index);
                    drop(current_time_tick);
                    let received_message = current_receiver.recv().unwrap();
                    let time_tick = station_time_tick.lock().unwrap();
                    println!("Station {}. Message Received", station_index);
                    if let StationMessages::InitPassengerList(mut list) = received_message {
                      println!("Station: {station_index} Message: {list:#?}");
                      let mut rejected_passenger_indeces = Vec::new();
                        for (index, passenger) in list.iter().enumerate() {
                          // let passenger_bus_route_list =
                            //     station_thread_passenger_bus_route_list.lock().unwrap();
                            // println!("Passenger will be added to station {}", station_index);
                            current_station
                                .add_passenger(
                                  passenger.clone().into(),
                                    *time_tick,
                                    &station_thread_passenger_bus_route_list.lock().unwrap(),
                                  )
                                  .unwrap_or_else(|passenger| {
                                    rejected_passenger_indeces.push(index);
                                    println!("Passenger {:?} had no valid routes", passenger);
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

                drop(current_time_tick);

                let received_message = current_receiver.recv().unwrap();
                // With test data, only one station recieves any bus arrived messages, for some reason
                // Note: It may be a good idea to make sure that stations with out passengers are not blocked, but this might work
                println!("Station {}. Not tick 1.", station_index);
                
                // time tick is kept by deadlock
                let time_tick = station_time_tick.lock().unwrap();
                println!("Unlocked time tick: {}", *time_tick);
                match received_message {
                  StationMessages::InitPassengerList(message) => panic!(
                        "PassengerInit message should not be sent on any time tick besides tick 1. Time tick: {}, List sent: {:#?}", time_tick, message 
                    ),
                    StationMessages::BusArrived{passengers_onboarding, bus_info}=> {
                      for passenger in passengers_onboarding.clone().iter_mut() {
                        let passenger_location = passenger.bus_schedule_iterator.next().unwrap();
                        passenger.archived_stop_list.push(passenger_location);
                      }
                      println!("Passengers Onboarding to bus {:?}: {:?}", bus_info.bus_index,passengers_onboarding);
                      current_station.passengers.extend(passengers_onboarding);
                      let bus_index = bus_info.bus_index;
                      println!("Bus {bus_index} arrived at station {station_index}.");
                      current_station.docked_buses.push(bus_info);
                      send_to_bus_channels[bus_index]
                        .send(StationToBusMessages::AcknowledgeArrival())
                        .unwrap();                          
                    },
                        
                    StationMessages::BusDeparted{bus_index } => {
                      println!("Recieved Departure Message early");
                      current_station.docked_buses.retain(|bus| {bus.bus_index != bus_index});
                    }
                 }

                      drop(time_tick);
                      
                      
                      let mut array_with_locations = current_station.docked_buses.iter().map(|bus| (bus.bus_index, Vec::<Passenger>::new()));
                      let mut next_passengers_for_buses_array: [Option<Vec<Passenger>>; NUM_OF_BUSES] = std::array::from_fn(|_| None); 
                      
                      
                      //next_passengers_for_buses_array = 
                      
                      println!("Array with locations for station {:?}: {:?}", &current_station.location.index,  &array_with_locations);
                      
                      let mut next_vec = array_with_locations.next();
                      println!("next_vec: {:?}", next_vec);
                      next_passengers_for_buses_array = next_passengers_for_buses_array.into_iter().enumerate().map(|(current_index, _)| {
                        
                        if let Some((old_index, vector)) = &mut next_vec {
                          if &current_index == old_index {
                            let next_vec_vector = std::mem::take(vector);
                            next_vec = array_with_locations.next();
                            Some(next_vec_vector)
                          } else {
                            None
                          }
                          
                        } else {
                          None
                        }
                        

                      }).collect::<Vec<_>>().try_into().unwrap();

                      // dbg!(&next_passengers_for_buses_array);
                      

                      println!("Station {} Passengers: {:?}", station_index, current_station.passengers);
                      
                      // Somehow, bus needs to send passengers to currently docked buses
                      
                      // TODO: Use a more efficient method than partition. Also, remove the clone, so peek actully gives an advantage.
                      // Why does passengers_for_next_destionation have no elements? At this point, pretty much all the passengers should have more stations to stop at. Shouldn't they be added to that list?
                      let (passengers_for_next_destination, arrived_passengers): (Vec<_>, Vec<_>) = current_station.passengers.iter_mut().partition(| passenger| { passenger.bus_schedule_iterator.clone().peek().is_some()}); 
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

                        
                        
                      
                        if  let Some(ref mut passengers) = next_passengers_for_buses_array[next_bus_index] {
                          passengers.push(passenger.clone());
                        } else {remaining_passengers.push(passenger.clone())}
                        
                        
                        
                      }
                      dbg!(&next_passengers_for_buses_array);

                      let docked_buses = &(current_station.docked_buses.clone());
                      
                      for bus in docked_buses {
                        let time_tick = station_time_tick.lock().unwrap();
                        println!("Station loop beginning.");
                        println!("Station Bus Time tick: {}", time_tick);
                        println!("Station: {}", current_station.location.index);
                        println!("Bus: {}", bus.bus_index);
                        let mut passengers_to_send = Vec::new();
                        let bus_index = bus.bus_index;
                        let remaining_capacity = bus.capacity_remaining;
                        // The index does not exist in the array - even though the index should be of a docked bus
                        let mut new_passenger_list = next_passengers_for_buses_array[bus_index].clone().unwrap_or_else(||  panic!("Bus {bus_index} should be docked at the station {}", current_station.location.index));
                        if new_passenger_list.len() > remaining_capacity {
                          let (passengers_to_add, rejected_passengers) = new_passenger_list.split_at(remaining_capacity);
                          passengers_overflowed.append(rejected_passengers.to_vec().as_mut());
                          passengers_to_send.append(passengers_to_add.to_vec().as_mut());
                          current_station.buses_unavailable.push(bus.bus_index);
                        } else {
                          passengers_to_send.append(&mut new_passenger_list);
                        }
                        
                        println!("Passengers to send: {:?}", passengers_to_send);

                        send_to_bus_channels[bus_index].send(StationToBusMessages::SendPassengers(passengers_to_send)).unwrap();
                        
                        let current_bus_route_list = bus_route_list.lock().unwrap();
                        let bus_route_vec: Vec<_> = current_bus_route_list.clone().into_iter().collect();
                        // bus_route_vec[0][0].

                        drop(current_bus_route_list);
                        
                        for passenger in passengers_overflowed.clone() {
                          let unavailable_buses = current_station.buses_unavailable.clone();
                          current_station.add_passenger_check_available_buses(passenger, *time_tick, &station_thread_passenger_bus_route_list.lock().unwrap(), unavailable_buses).unwrap();
                        }

                        drop(time_tick);

                        
                        
                        println!("Pre-bus departure");
                       
                        send_to_bus_channels[bus_index].send(StationToBusMessages::RequestDeparture()).unwrap();

                        println!("Departure message sent");

                        let departure_acknowledge_messsage = current_receiver.recv().unwrap();
                        let StationMessages::BusDeparted { bus_index: message_bus_index } = departure_acknowledge_messsage else {
                          panic!("Invalid message recieved. Expected StationMessages::GrantDeparture recieved {:?}", departure_acknowledge_messsage);
                        };

                        current_station.docked_buses.retain(|bus| {bus.bus_index != message_bus_index});






                      }
                      // current_station.passengers = passengers_for_next_destination;

            

              }
        });

        station_handle
  }