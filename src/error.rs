use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum TimeoutIteratorError {
    ErrorSpawningThread(std::io::Error),
    TimedOut,
    Disconnected,
}
impl Error for TimeoutIteratorError {}
impl fmt::Display for TimeoutIteratorError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "TimeoutIteratorError:: {}",
            match self {
                TimeoutIteratorError::TimedOut =>
                    "Timed out waiting on the underlying iterator for the next item".to_owned(),
                TimeoutIteratorError::Disconnected =>
                    "Underlying iterator closed/disconnected".to_owned(),
                TimeoutIteratorError::ErrorSpawningThread(e) => format!(
                    "Error when spawing a thread for sinking events. Inner io::Error: {}",
                    e
                ),
            }
        )
    }
}
impl From<std::io::Error> for TimeoutIteratorError {
    fn from(err: std::io::Error) -> TimeoutIteratorError {
        TimeoutIteratorError::ErrorSpawningThread(err)
    }
}
