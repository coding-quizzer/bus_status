use std::fmt::{Display, Formatter};

use crate::TimeTick;

pub struct TerminalMessage {
    pub content: TerminalType,
    pub time_tick: TimeTick,
    pub station_index: usize,
}

impl Display for TerminalMessage {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{} - {}", self.content, self.time_tick)
    }
}
pub enum TerminalType {
    InitiatedPassenger(InitiatedPassengerInfo),
    WaitingPassenger(WaitingPassengerInfo),
    ArrivedPassenger(ArrivedPassengerInfo),
    BoardedPassenger(BoardedPassengerInfo),
    RejectedPassenger(RejectedPassengerInfo),
    StrandedPassenger(StrandedPassengerInfo),
    NoPassengerFromStation { passenger_index: usize },
}

impl Display for TerminalType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            TerminalType::InitiatedPassenger(info) => write!(f, "{info}"),
            TerminalType::ArrivedPassenger(info) => write!(f, "{info}"),
            TerminalType::BoardedPassenger(info) => write!(f, "{info}"),
            TerminalType::RejectedPassenger(info) => write!(f, "{info}"),
            TerminalType::StrandedPassenger(info) => write!(f, "{info}"),
            TerminalType::WaitingPassenger(info) => write!(f, "{info}"),
            TerminalType::NoPassengerFromStation {
                passenger_index: index,
            } => write!(
                f,
                "Station {index} processes no passengers in the current time tick"
            ),
        }
    }
}

pub struct ArrivedPassengerInfo {
    pub passenger_index: usize,
    station_location: crate::Location,
    pub final_location: bool,
}

pub struct BoardedPassengerInfo {
    pub passenger_index: usize,
    bus_number: usize,
}

pub struct RejectedPassengerInfo {
    pub passenger_index: usize,
    bus_number: usize,
}

pub struct StrandedPassengerInfo {
    pub passenger_index: usize,
    current_station_index: usize,
    destination_location_index: usize,
}

pub struct WaitingPassengerInfo {
    pub passenger_index: usize,
    location_index: usize,
}

pub struct InitiatedPassengerInfo {
    pub passenger_index: usize,
    location_index: usize,
}

impl ArrivedPassengerInfo {
    pub fn new_layover(
        passenger_index: usize,
        station_location: crate::Location,
    ) -> ArrivedPassengerInfo {
        ArrivedPassengerInfo {
            passenger_index,
            station_location,
            final_location: false,
        }
    }
    pub fn new_final(
        passenger_index: usize,
        station_location: crate::Location,
    ) -> ArrivedPassengerInfo {
        ArrivedPassengerInfo {
            passenger_index,
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
                self.passenger_index, self.station_location
            )
        } else {
            write!(
                f,
                "Passenger {} arrived at intermediate location: Location {}",
                self.passenger_index, self.station_location
            )
        }
    }
}

impl BoardedPassengerInfo {
    pub fn new(passenger_index: usize, bus_number: usize) -> BoardedPassengerInfo {
        BoardedPassengerInfo {
            passenger_index,
            bus_number,
        }
    }
}

impl Display for BoardedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} boarded bus {}",
            self.passenger_index, self.bus_number
        )
    }
}

impl RejectedPassengerInfo {
    pub fn new(passenger_index: usize, bus_number: usize) -> RejectedPassengerInfo {
        RejectedPassengerInfo {
            passenger_index,
            bus_number,
        }
    }
}

impl Display for RejectedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} rejected from Bus {} because it was already at capacity",
            self.passenger_index, self.bus_number
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
            passenger_index: index,
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
            self.passenger_index, self.current_station_index, self.destination_location_index
        )
    }
}
impl WaitingPassengerInfo {
    pub fn new(passenger_index: usize, location_index: usize) -> WaitingPassengerInfo {
        WaitingPassengerInfo {
            passenger_index,
            location_index,
        }
    }
}
impl Display for WaitingPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} waiting in station {}",
            self.passenger_index, self.location_index,
        )
    }
}

impl InitiatedPassengerInfo {
    pub fn new(index: usize, location_index: usize) -> InitiatedPassengerInfo {
        InitiatedPassengerInfo {
            passenger_index: index,
            location_index,
        }
    }
}

impl Display for InitiatedPassengerInfo {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Passenger {} initiated in station {}",
            self.passenger_index, self.location_index,
        )
    }
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PassengerState {
    // Uninitialized,
    Unprocessed,
    Boarded,
    Processed,
    Finished,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StationState {
    Unprocessed,
    NoPassengers,
    Processed,
}
