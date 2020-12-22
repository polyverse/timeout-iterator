use std::time::Duration;

use crate::error;
use std::sync::mpsc;
use std::thread;

pub struct TimeoutIterator<T> {
    source: mpsc::Receiver<T>,
    buffer: Vec<T>,
}

impl<T> TimeoutIterator<T>
where
    T: Send + 'static,
{
    pub fn with_iter<R>(iter: R) -> Result<TimeoutIterator<T>, error::TimeoutIteratorError>
    where
        R: Iterator<Item = T> + Send + 'static,
    {
        let (sink, source): (mpsc::Sender<T>, mpsc::Receiver<T>) = mpsc::channel();

        thread::Builder::new().name("TimeoutIterator::sender".to_owned()).spawn(move || {
            for item in iter {
                if let Err(e) = sink.send(item) {
                    eprintln!("TimeoutIterator:: Error sending data to channel. Receiver may have closed. Closing up sender. Error: {}", e);
                    return;
                }
            }
        })?;

        Ok(TimeoutIterator {
            source,
            buffer: Vec::new(),
        })
    }

    pub fn next_timeout(&mut self, timeout: Duration) -> Result<T, error::TimeoutIteratorError> {
        if !self.buffer.is_empty() {
            return Ok(self.buffer.remove(0));
        };

        match self.source.recv_timeout(timeout) {
            Ok(item) => Ok(item),
            Err(e) => match e {
                mpsc::RecvTimeoutError::Timeout => Err(error::TimeoutIteratorError::TimedOut),
                mpsc::RecvTimeoutError::Disconnected => {
                    Err(error::TimeoutIteratorError::Disconnected)
                }
            },
        }
    }

    pub fn peek_timeout(&mut self, timeout: Duration) -> Result<&T, error::TimeoutIteratorError> {
        if self.buffer.is_empty() {
            match self.source.recv_timeout(timeout) {
                Ok(item) => self.buffer.push(item),
                Err(e) => match e {
                    mpsc::RecvTimeoutError::Timeout => {
                        return Err(error::TimeoutIteratorError::TimedOut)
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        return Err(error::TimeoutIteratorError::Disconnected)
                    }
                },
            }
        };

        Ok(self.buffer.first().unwrap())
    }

    pub fn peek(&mut self) -> Option<&T> {
        if self.buffer.is_empty() {
            match self.next() {
                Some(item) => self.buffer.push(item),
                None => {
                    return None;
                }
            }
        };

        Some(self.buffer.first().unwrap())
    }
}

impl<T> Iterator for TimeoutIterator<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        };

        match self.source.recv() {
            Ok(item) => Some(item),
            Err(e) => {
                eprintln!(
                    "TimeoutIterator:: Error occurred reading from source: {}.",
                    e
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::prelude::*;

    #[test]
    fn iterates() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();

        let mut ti = TimeoutIterator::with_iter(lines_iterator).unwrap();

        assert_eq!(ti.next().unwrap().unwrap(), "1");
        assert_eq!(ti.next().unwrap().unwrap(), "2");
        assert_eq!(ti.next().unwrap().unwrap(), "3");
        assert_eq!(ti.next().unwrap().unwrap(), "4");
        assert_eq!(ti.next().unwrap().unwrap(), "5");
    }

    #[test]
    fn next_timeout() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();

        let mut ti = TimeoutIterator::with_iter(lines_iterator).unwrap();

        assert_eq!(ti.next().unwrap().unwrap(), "1");
        assert_eq!(ti.next().unwrap().unwrap(), "2");
        assert_eq!(ti.next().unwrap().unwrap(), "3");
        assert_eq!(ti.next().unwrap().unwrap(), "4");
        assert_eq!(ti.next().unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }

    #[test]
    fn peek_timeout_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();
        let mut ti = TimeoutIterator::with_iter(lines_iterator).unwrap();

        assert_eq!(ti.next().unwrap().unwrap(), "1");
        assert_eq!(ti.next().unwrap().unwrap(), "2");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .ok()
                .unwrap()
                .as_ref()
                .unwrap(),
            "3"
        );
        assert_eq!(ti.next().unwrap().unwrap(), "3");
        assert_eq!(ti.next().unwrap().unwrap(), "4");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .ok()
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .ok()
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(ti.next().unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }

    #[test]
    fn peek_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();
        let mut ti = TimeoutIterator::with_iter(lines_iterator).unwrap();

        assert_eq!(ti.next().unwrap().unwrap(), "1");
        assert_eq!(ti.next().unwrap().unwrap(), "2");
        assert_eq!(ti.peek().unwrap().as_ref().unwrap(), "3");
        assert_eq!(ti.next().unwrap().unwrap(), "3");
        assert_eq!(ti.next().unwrap().unwrap(), "4");
        assert_eq!(ti.peek().unwrap().as_ref().unwrap(), "5");
        assert_eq!(ti.peek().unwrap().as_ref().unwrap(), "5");
        assert_eq!(ti.next().unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }

    #[test]
    fn item_iterator() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];
        let mut ti = TimeoutIterator::with_iter(numbers.into_iter()).unwrap();

        assert_eq!(ti.next().unwrap(), 1);
        assert_eq!(ti.next().unwrap(), 2);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), 3);
        assert_eq!(ti.next().unwrap(), 3);
        assert_eq!(ti.next().unwrap(), 4);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), 5);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), 5);
        assert_eq!(ti.next().unwrap(), 5);

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }

    #[test]
    fn is_sendable() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];
        let mut ti = TimeoutIterator::with_iter(numbers.into_iter()).unwrap();
        thread::spawn(move || {
            ti.next();
        });
        assert!(
            true,
            "If this compiles, TimeoutIterator is Send'able across threads."
        );
    }
}
