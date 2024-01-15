use bus_system;
use bus_system::data;
use std::path::Path;
#[test]
fn can_find_basic_route() {
    let input_data = data::read_data_from_file(Path::new("test_data/simple_data.json"));
}
