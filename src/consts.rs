use std::sync::Once;
pub static INIT_VALUES: Once = Once::new();
pub const DEFAULT_BUS_CAPACITY: usize = 10;
pub const GLOBAL_PASSENGER_COUNT: usize = 10;

pub const DEFAULT_GLOBAL_LOCATION_COUNT: usize = 5;
pub const DEFAULT_NUM_OF_BUSES: usize = 3;
pub const NUM_STOPS_PER_BUS: usize = 3;
pub const MAX_LOCATION_DISTANCE: u32 = 3;
pub const READ_JSON: bool = option_env!("READ_DATA").is_some();
pub const WRITE_JSON: bool = option_env!("WRITE_DATA").is_some();
