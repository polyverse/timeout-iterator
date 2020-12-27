pub mod error;

#[cfg(all(test, feature = "async"))]
#[macro_use]
extern crate assert_matches;

#[cfg(feature = "sync")]
pub mod synchronous;

#[cfg(feature = "async")]
pub mod asynchronous;
