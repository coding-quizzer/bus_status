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
        let passenger = Passenger::new(location_vector[0], location_vector[1]);
        passenger_list.push(passenger);
    }

    let first_bus_route = vec![
        BusLocation {
            location: location_vector[0],
            distance_to_location: 1,
        },
        BusLocation {
            location: location_vector[1],
            distance_to_location: 1,
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

    // TODO: return the passenger lists so that this part of the test is meaningful
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
