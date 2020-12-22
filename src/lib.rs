pub mod error;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(not(feature = "async"))]
pub mod synchronous;

#[cfg(feature = "async")]
pub mod asynchronous;
