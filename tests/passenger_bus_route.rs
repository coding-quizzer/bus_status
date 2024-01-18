use bus_system::data;
use bus_system::location::{BusLocation, Location};
use bus_system::passenger::{Passenger, PassengerOnboardingBusSchedule};
use bus_system::{
    calculate_passenger_schedule_for_bus, convert_bus_route_list_to_passenger_bus_route_list,
};
use std::path::Path;
use uuid::uuid;

#[test]
fn can_find_basic_route() {
    let input_data =
        data::read_data_from_file(Path::new("./tests/test_data/simple_data.json")).unwrap();
    let bus_routes = input_data.bus_routes;
    let passenger_facing_bus_routes = bus_routes
        .into_iter()
        .map(convert_bus_route_list_to_passenger_bus_route_list)
        .collect::<Vec<_>>();
    let test_passenger: Passenger = Into::<Passenger>::into(input_data.passengers[0].clone());
    let calculated_passenger_bus_route =
        calculate_passenger_schedule_for_bus(test_passenger, 0, &passenger_facing_bus_routes);
    let expected_passenger_bus_route = [
        PassengerOnboardingBusSchedule {
            time_tick: 4,
            bus_num: Some(2),
            stop_location: Location {
                index: 1,
                id: uuid!("0adf8f60-3748-4af0-a985-5eb0de081d70"),
            },
        },
        PassengerOnboardingBusSchedule {
            time_tick: 8,
            bus_num: None,
            stop_location: Location {
                index: 3,
                id: uuid!("135f70d1-0f07-4a0f-a996-3412019ea584"),
            },
        },
    ];
    assert_eq!(
        calculated_passenger_bus_route.unwrap(),
        expected_passenger_bus_route
    );
}
