use bus_system::consts::DEFAULT_BUS_CAPACITY;
use bus_system::location::{BusLocation, Location};
use bus_system::main_loop::run_simulation;
use bus_system::main_loop::ConfigStruct;
use bus_system::passenger::Passenger;
#[test]
fn rejected_passengers_and_arrived_passengers_are_accounted_for() {
    const TEST_PASSENGER_COUNT: usize = 20;

    let location_vector = vec![Location::new(0), Location::new(1)];
    let mut passenger_list = vec![];
    for _ in 0..TEST_PASSENGER_COUNT {
        let passenger = Passenger::new(location_vector[0], location_vector[1], 0);
        passenger_list.push(passenger);
    }

    // distance_to_location must be a multiple of 2
    let first_bus_route = vec![
        BusLocation {
            location: location_vector[0],
            distance_to_location: 2,
        },
        BusLocation {
            location: location_vector[1],
            distance_to_location: 2,
        },
    ];

    let all_bus_routes = vec![first_bus_route];

    let config_struct = ConfigStruct {
        num_of_buses: 1,
        num_of_passengers: TEST_PASSENGER_COUNT,
        num_of_locations: 2,
        bus_capacity: 10,
    };

    let final_passengers_list = run_simulation(
        location_vector,
        passenger_list,
        all_bus_routes,
        config_struct,
    );

    let passenger_count_at_station = final_passengers_list
        .location_lists
        .iter()
        .flatten()
        .count();
    let remaining_passengers_count = final_passengers_list.remaining_passengers.len();
    assert_eq!(
        passenger_count_at_station + remaining_passengers_count,
        TEST_PASSENGER_COUNT
    );
}

#[test]
// All passengers are included regardless of the time tick they arrive at the destination station
fn passengers_from_all_timeticks_are_accounted_for() {
    const TEST_PASSENGER_COUNT: usize = 20;

    let location_vector = vec![Location::new(0), Location::new(1), Location::new(2)];
    let mut passenger_list = vec![];
    for current_index in 0..TEST_PASSENGER_COUNT {
        // Half the passengers are going to destination 1, half go tp destination 2
        let destination_location = if current_index % 2 == 0 {
            location_vector[1]
        } else {
            location_vector[2]
        };
        let passenger = Passenger::new(location_vector[0], destination_location, 0);
        passenger_list.push(passenger);
    }

    // distance_to_location must be a multiple of 2
    let first_bus_route = vec![
        BusLocation {
            location: location_vector[0],
            distance_to_location: 2,
        },
        BusLocation {
            location: location_vector[1],
            distance_to_location: 2,
        },
        BusLocation {
            location: location_vector[2],
            distance_to_location: 2,
        },
    ];

    let all_bus_routes = vec![first_bus_route];

    let config_struct = ConfigStruct {
        num_of_buses: 1,
        num_of_passengers: TEST_PASSENGER_COUNT,
        num_of_locations: 3,
        bus_capacity: 20,
    };

    let final_passengers_list = run_simulation(
        location_vector,
        passenger_list,
        all_bus_routes,
        config_struct,
    );

    let passenger_count_at_station = final_passengers_list
        .location_lists
        .iter()
        .flatten()
        .count();
    let remaining_passengers_count = final_passengers_list.remaining_passengers.len();
    assert_eq!(
        passenger_count_at_station + remaining_passengers_count,
        TEST_PASSENGER_COUNT
    );
}
