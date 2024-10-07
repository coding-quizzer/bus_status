use crate::bus::SendableBus;
use crate::passenger::Passenger;
#[derive(PartialEq, Debug)]
// Messages to the station
pub enum StationMessages {
    InitPassengerList(Vec<Passenger>),
    BusArrived {
        passengers_onboarding: Vec<Passenger>,
        bus_info: SendableBus,
    },
    BusDeparted {
        bus_index: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum StationToPassengersMessages {
    ConfirmInitPassengerList(usize),
}

#[derive(Debug)]
pub enum StationToBusMessages {
    AcknowledgeArrival(),
    SendPassengers(Vec<Passenger>),
    FinishedUnloading,
    RequestDeparture,
    StationRemovedBus,
}

#[derive(PartialEq, Debug)]
pub enum BusMessages {
    InitBus { bus_index: usize },
    InitPassengers,
    BusPanicked { bus_index: usize, message: String },
    AdvanceTimeStepForMovingBus { bus_index: usize },
    AdvanceTimeStepForUnloadedBus { bus_index: usize },
    AdvanceTimeStepForLoadedBus { bus_index: usize },
    BusFinished { bus_index: usize },
}

#[derive(Debug)]
pub enum ProgramEndType {
    ProgramFinished,
    ProgramCrashed { message: String },
}

#[derive(Debug)]
pub enum SyncToStationMessages {
    AdvanceTimeStep(crate::TimeTick),
    ProgramFinished(ProgramEndType),
}

#[derive(Debug)]
pub enum StationToSyncMessages {
    CrashProgram { message: String },
}

#[derive(Debug)]
pub enum SyncToBusMessages {
    AdvanceTimeStep(crate::TimeTick),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BusThreadStatus {
    Uninitialized,
    Moving,
    BusFinishedRoute,
    FinishedUnloadingPassengers,
    FinishedLoadingPassengers,
    WaitingForTimeStep,
}
