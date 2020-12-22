use std::fmt;
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum Error {
    ErrorSpawningThread(std::io::Error),
    TimedOut,
    Disconnected,
}

impl std::error::Error for Error {}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "TimeoutIteratorError:: {}",
            match self {
                Self::TimedOut =>
                    "Timed out waiting on the underlying iterator for the next item".to_owned(),
                Self::Disconnected => "Underlying iterator closed/disconnected".to_owned(),
                Self::ErrorSpawningThread(e) => format!(
                    "Error when spawing a thread for sinking events. Inner io::Error: {}",
                    e
                ),
            }
        )
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::ErrorSpawningThread(err)
    }
}
