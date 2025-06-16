use std::fmt::{Display, Formatter};
pub enum TerminalMessage {
    ArrivedPassenger(ArrivedPassengerInfo),
    BoardedPassenger(BoardedPassengerInfo),
    StrandedPassenger(StrandedPassengerInfo),
}

impl std::fmt::Display for TerminalMessage {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            TerminalMessage::ArrivedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::BoardedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::StrandedPassenger(info) => write!(f, "{info}"),
        }
    }
}

pub struct ArrivedPassengerInfo {
    index: u32,
    station_location: crate::Location,
    final_location: bool,
}

pub struct BoardedPassengerInfo {
    index: u32,
    bus_number: u32,
}

pub struct StrandedPassengerInfo {
    index: u32,
    location_index: u32,
}

impl std::fmt::Display for ArrivedPassengerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        if self.final_location {
            write!(
                f,
                "Passenger {} arrived at destination location: Location {}",
                self.index, self.station_location
            )
        } else {
            write!(
                f,
                "Passenger {} arrived at intermediate location: Location {}",
                self.index, self.station_location
            )
        }
    }
}

impl Display for BoardedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} boarded bus {}",
            self.index, self.bus_number
        )
    }
}

impl Display for StrandedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} failed to find valid route to destination {}",
            self.index, self.location_index
        )
    }
}
