use crate::bus::SendableBus;
use crate::Passenger;
#[derive(PartialEq, Debug)]
pub enum RejectedPassengersMessages {
    MovingBus,
    StoppedBus { rejected_passengers: Vec<Passenger> },
    CompletedProcessing,
}

#[derive(Debug)]
pub enum StationMessages {
    InitPassengerList(Vec<Passenger>),
    BusArrived(SendableBus),
}

#[derive(Debug, PartialEq, Eq)]
pub enum StationToPassengersMessages {
    ConfirmInitPassengerList(usize),
}
pub enum StationToBusMessages {
    AcknowledgeArrival(),
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
