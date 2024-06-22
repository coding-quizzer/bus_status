use bus_system::consts::BUS_CAPACITY;
use bus_system::location::{BusLocation, Location};
use bus_system::main_loop::main_loop;
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

    let all_bus_routes = [first_bus_route, vec![], vec![]];

    // FIXME: main_loop is not finishing here for some reason
    let final_passengers_list = main_loop(location_vector, passenger_list, all_bus_routes);
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
