use crate::bus::SendableBus;
use crate::passenger::Passenger;
#[derive(PartialEq, Debug)]
pub enum RejectedPassengersMessages {
    MovingBus,
    StoppedBus { rejected_passengers: Vec<Passenger> },
    CompletedProcessing,
}

#[derive(Debug)]
// Messages to the station
pub enum StationMessages {
    InitPassengerList(Vec<Passenger>),
    BusArrived {
        passengers_onboarding: Vec<Passenger>,
        bus_info: SendableBus,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum StationToPassengersMessages {
    ConfirmInitPassengerList(usize),
}
pub enum StationToBusMessages {
    AcknowledgeArrival(),
    SendPassengers(Vec<Passenger>),
}

#[derive(PartialEq, Debug)]
pub enum BusMessages {
    InitBus {
        bus_index: usize,
    },
    InitPassengers,
    AdvanceTimeStep {
        // current_time_step: u32,
        bus_index: usize,
    },
    BusFinished {
        bus_index: usize,
    },

    RejectedPassengers(RejectedPassengersMessages),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BusThreadStatus {
    Uninitialized,
    BusFinishedRoute,
    WaitingForTimeStep,
    CompletedTimeStep,
}
