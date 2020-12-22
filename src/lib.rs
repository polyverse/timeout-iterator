pub mod error;

#[cfg(not(feature = "async"))]
pub mod synchronous;

#[cfg(feature = "async")]
pub mod asynchronous;
