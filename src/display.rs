use std::fmt::{Display, Formatter};
pub enum TerminalMessage {
    InitiatedPassenger(InitiatedPassengerInfo),
    WaitingPassenger(WaitingPassengerInfo),
    ArrivedPassenger(ArrivedPassengerInfo),
    BoardedPassenger(BoardedPassengerInfo),
    RejectedPassenger(RejectedPassengerInfo),
    StrandedPassenger(StrandedPassengerInfo),
}

impl std::fmt::Display for TerminalMessage {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            TerminalMessage::InitiatedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::ArrivedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::BoardedPassenger(info) => write!(f, "{info}"),
            TerminalMessage::RejectedPassenger(info) => write!(f, "{info}"),
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
    index: usize,
    bus_number: usize,
}

pub struct RejectedPassengerInfo {
    index: usize,
    bus_number: usize,
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

impl BoardedPassengerInfo {
    pub fn new(passenger_index: usize, bus_number: usize) -> BoardedPassengerInfo {
        BoardedPassengerInfo {
            index: passenger_index,
            bus_number,
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

impl RejectedPassengerInfo {
    pub fn new(passenger_index: usize, bus_number: usize) -> RejectedPassengerInfo {
        RejectedPassengerInfo {
            index: passenger_index,
            bus_number,
        }
    }
}

impl Display for RejectedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} rejected from Bus {} because it was already at capacity",
            self.index, self.bus_number
        )
    }
}

impl StrandedPassengerInfo {
    pub fn new(
        index: usize,
        current_station_index: usize,
        destination_location_index: usize,
    ) -> StrandedPassengerInfo {
        StrandedPassengerInfo {
            index,
            current_station_index,
            destination_location_index,
        }
    }
}

impl Display for StrandedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} stuck at station {}. Failed to find valid route to destination {}",
            self.index, self.current_station_index, self.destination_location_index
        )
    }
}
impl WaitingPassengerInfo {
    pub fn new(passenger_index: usize, location_index: usize) -> WaitingPassengerInfo {
        WaitingPassengerInfo {
            index: passenger_index,
            location_index,
        }
    }
}
impl Display for WaitingPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
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
        write!(
            f,
            "Passenger {} initiated in station {}",
            self.index, self.location_index,
        )
    }
}
