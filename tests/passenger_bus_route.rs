use bus_system::location::{Location, PassengerBusLocation};
use bus_system::passenger::{Passenger, PassengerOnboardingBusSchedule};
use bus_system::{
    calculate_passenger_schedule_for_bus, convert_bus_route_list_to_passenger_bus_route_list,
};
use bus_system::{calculate_passenger_schedule_for_bus_check_available_buses, data};
use std::path::Path;
use uuid::uuid;

fn get_passenger_bus_routes_from_input_data(
    input_file: &Path,
) -> (Vec<Vec<PassengerBusLocation>>, Vec<Location>) {
    let full_path = Path::new("./tests/test_data/").join(input_file);
    let full_path = full_path.as_ref();
    let input_data = data::read_data_from_file(full_path).unwrap();
    let bus_routes = input_data.bus_routes;
    let passenger_facing_bus_route = bus_routes
        .into_iter()
        .map(convert_bus_route_list_to_passenger_bus_route_list)
        .collect::<Vec<_>>();
    (passenger_facing_bus_route, input_data.location_vector)
}

#[test]
fn can_find_basic_route() {
    let (passenger_facing_bus_routes, location_list) =
        get_passenger_bus_routes_from_input_data(Path::new("simple_data.json"));
    let test_passenger: Passenger = Passenger::new(location_list[3], location_list[2]);
    let calculated_passenger_bus_route =
        calculate_passenger_schedule_for_bus(&test_passenger, 0, &passenger_facing_bus_routes);
    let expected_passenger_bus_route = vec![
        PassengerOnboardingBusSchedule {
            time_tick: 4,
            bus_num: Some(0),
            stop_location: location_list[3],
        },
        PassengerOnboardingBusSchedule {
            time_tick: 8,
            bus_num: None,
            stop_location: location_list[2],
        },
    ];
    assert_eq!(
        calculated_passenger_bus_route,
        Ok(expected_passenger_bus_route),
    );
}

#[test]
fn can_find_bus_route_with_transfer() {
    let (passenger_facing_bus_routes, location_list) =
        get_passenger_bus_routes_from_input_data(Path::new("simple_data.json"));
    let test_passenger = Passenger::new(location_list[0], location_list[2]);
    let calculated_passenger_bus_route =
        calculate_passenger_schedule_for_bus(&test_passenger, 0, &passenger_facing_bus_routes);

    let expected_passenger_route = vec![
        PassengerOnboardingBusSchedule {
            time_tick: 4,
            bus_num: Some(1),
            stop_location: location_list[0],
        },
        PassengerOnboardingBusSchedule {
            time_tick: 8,
            bus_num: Some(2),
            stop_location: location_list[3],
        },
        PassengerOnboardingBusSchedule {
            time_tick: 12,
            bus_num: None,
            stop_location: location_list[2],
        },
    ];

    assert_eq!(calculated_passenger_bus_route, Ok(expected_passenger_route));
}

#[test]
fn finds_shortest_route() {
    let (passenger_facing_bus_routes, location_list) =
        get_passenger_bus_routes_from_input_data(Path::new("data_with_distances.json"));
    let test_passenger = Passenger::new(location_list[1], location_list[2]);
    let calculated_passenger_bus_route =
        calculate_passenger_schedule_for_bus(&test_passenger, 0, &passenger_facing_bus_routes);

    let expected_passenger_route = vec![
        PassengerOnboardingBusSchedule {
            time_tick: 6,
            bus_num: Some(2),
            stop_location: location_list[1],
        },
        PassengerOnboardingBusSchedule {
            // time tick represents time tick when bus 3 drops the passenger off, rather than when  bus 0 picks passenger up
            time_tick: 14,
            bus_num: Some(0),
            stop_location: location_list[3],
        },
        PassengerOnboardingBusSchedule {
            time_tick: 22,
            bus_num: None,
            stop_location: location_list[2],
        },
    ];

    assert_eq!(calculated_passenger_bus_route, Ok(expected_passenger_route),);
}

// Use --no-capture to ensure stdout is displayed
#[test]
fn special_route_removing_some_bus_locations() {
    let (passenger_facing_bus_routes, location_list) =
        get_passenger_bus_routes_from_input_data(Path::new("simple_data.json"));
    let test_passenger = Passenger::new(location_list[3], location_list[2]);
    let calculated_passenger_bus_route = calculate_passenger_schedule_for_bus_check_available_buses(
        &test_passenger,
        4,
        &passenger_facing_bus_routes,
        vec![0],
    );

    let expected_passenger_route = vec![
        PassengerOnboardingBusSchedule {
            time_tick: 8,
            bus_num: Some(2),
            stop_location: location_list[3],
        },
        PassengerOnboardingBusSchedule {
            // time tick represents time tick when bus 3 drops the passenger off, rather than when  bus 0 picks passenger up
            time_tick: 12,
            bus_num: None,
            stop_location: location_list[2],
        },
    ];

    assert_eq!(calculated_passenger_bus_route, Ok(expected_passenger_route),);
}
