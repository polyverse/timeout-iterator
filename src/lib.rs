pub mod error;

#[cfg(feature = "sync")]
pub mod synchronous;

#[cfg(feature = "async")]
pub mod asynchronous;
