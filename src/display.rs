use std::fmt::{Display, Formatter};
pub enum TerminalMessage {
    InitiatedPassenger(InitiatedPassengerInfo),
    WaitingPassenger(WaitingPassengerInfo),
    ArrivedPassenger(ArrivedPassengerInfo),
    BoardedPassenger(BoardedPassengerInfo),
    StrandedPassenger(StrandedPassengerInfo),
}

impl std::fmt::Display for TerminalMessage {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            TerminalMessage::InitiatedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::ArrivedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::BoardedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::StrandedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::WaitingPassenger(info) => write!(f, "{info}"),
        }
    }
}

pub struct ArrivedPassengerInfo {
    index: usize,
    station_location: crate::Location,
    final_location: bool,
}

pub struct BoardedPassengerInfo {
    index: u32,
    bus_number: u32,
}

pub struct StrandedPassengerInfo {
    index: usize,
    current_station_index: usize,
    destination_location_index: usize,
}

pub struct WaitingPassengerInfo {
    index: usize,
    location_index: usize,
}

pub struct InitiatedPassengerInfo {
    index: usize,
    location_index: usize,
}

pub struct FailedInitiatePassenger {
    index: usize,
}

impl ArrivedPassengerInfo {
    pub fn new_layover(index: usize, station_location: crate::Location) -> ArrivedPassengerInfo {
        ArrivedPassengerInfo {
            index,
            station_location,
            final_location: false,
        }
    }
    pub fn new_final(index: usize, station_location: crate::Location) -> ArrivedPassengerInfo {
        ArrivedPassengerInfo {
            index,
            station_location,
            final_location: true,
        }
    }
}

impl std::fmt::Display for ArrivedPassengerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        if self.final_location {
            writeln!(
                f,
                "Passenger {} arrived at destination location: Location {}",
                self.index, self.station_location
            )
        } else {
            writeln!(
                f,
                "Passenger {} arrived at intermediate location: Location {}",
                self.index, self.station_location
            )
        }
    }
}

impl Display for BoardedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        writeln!(
            f,
            "Passenger {} boarded bus {}",
            self.index, self.bus_number
        )
    }
}

impl Display for WaitingPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        writeln!(
            f,
            "Passenger {} waiting in station {}",
            self.index, self.location_index,
        )
    }
}

impl InitiatedPassengerInfo {
    pub fn new(index: usize, location_index: usize) -> InitiatedPassengerInfo {
        InitiatedPassengerInfo {
            index,
            location_index,
        }
    }
}

impl Display for InitiatedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        writeln!(
            f,
            "Passenger {} initiated in station {}",
            self.index, self.location_index,
        )
    }
}

impl Display for FailedInitiatePassenger {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        writeln!(
            f,
            "Passenger {} failed to initiate due to no valid routes",
            self.index
        )
    }
}
