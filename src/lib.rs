#[macro_use]
extern crate doc_comment;

doc_comment!(include_str!("../README.md"));
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration};
use std::result;
use std::fmt;

pub enum TimeoutIteratorError {
    Timeout,
    Disconnected,
}

pub struct TimeoutIterator<T> {
    source: Receiver<T>,
    buffer: Vec<T>,
    verbosity: u8,
}

impl<T> TimeoutIterator<T> 
where T: Send + 'static {

    /**
     * Use this constructor 
     */
    pub fn from_result_iterator<R, E>(reader: R, verbosity: u8) -> TimeoutIterator<T>
    where R: Iterator<Item = result::Result<T, E>> + Send + 'static,
        E: fmt::Display + fmt::Debug {
        let (sink, source) : (Sender<T>, Receiver<T>) = mpsc::channel();

        thread::spawn(move || {
            for maybe_item in reader {
                match maybe_item {
                    Ok(item) => if let Err(e) = sink.send(item) {
                        if verbosity > 0 { eprintln!("TimeoutIterator:: Error sending data to channel. Receiver may have closed. Closing up sender. Error: {}", e); }
                        return;
                    },
                    Err(e) => {
                        if verbosity > 0 { eprintln!("TimeoutIterator:: Error reading from source iterator. Closing reader. Error: {}", e); }
                        return;
                    }
                }
            }
        });

        TimeoutIterator {
            source,
            verbosity,
            buffer: Vec::new()
        }
    }

    pub fn from_item_iterator<R>(reader: R, verbosity: u8) -> TimeoutIterator<T>
    where R: Iterator<Item = T> + Send + 'static {
        let (sink, source) : (Sender<T>, Receiver<T>) = mpsc::channel();

        thread::spawn(move || {
            for item in reader {
                if let Err(e) = sink.send(item) {
                    if verbosity > 0 { eprintln!("TimeoutIterator:: Error sending data to channel. Receiver may have closed. Closing up sender. Error: {}", e); }
                    return;
                }
            }
        });

        TimeoutIterator {
            source,
            verbosity,
            buffer: Vec::new()
        }
    }

    pub fn next_timeout(&mut self, timeout: Duration) -> Result<T, TimeoutIteratorError> {
        if !self.buffer.is_empty() {
            return Ok(self.buffer.remove(0));
        };

        match self.source.recv_timeout(timeout) {
            Ok(item) => Ok(item),
            Err(e) => match e {
                mpsc::RecvTimeoutError::Timeout => Err(TimeoutIteratorError::Timeout),
                mpsc::RecvTimeoutError::Disconnected => Err(TimeoutIteratorError::Disconnected)
            }
        }            
    }

    pub fn peek_timeout(&mut self, timeout: Duration) -> Result<&T, TimeoutIteratorError> {
        if self.buffer.is_empty() {
            match self.source.recv_timeout(timeout) {
                Ok(item) => self.buffer.push(item),
                Err(e) => match e {
                    mpsc::RecvTimeoutError::Timeout => return Err(TimeoutIteratorError::Timeout),
                    mpsc::RecvTimeoutError::Disconnected => return Err(TimeoutIteratorError::Disconnected)
                }
            }   
        };

        Ok(self.buffer.first().unwrap())
    }
}


impl<T> Iterator for TimeoutIterator<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        };

        match self.source.recv() {
            Ok(item) => {
                Some(item)
            },
            Err(e) => {
                if self.verbosity > 0 {
                    eprintln!("TimeoutIterator:: Error occurred reading from source: {}.", e);
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::prelude::*;
    use super::*;

    #[test]
    fn iterates() {
        let realistic_message = 
r"1
2
3
4
5";
        let lines_iterator = (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();

        let mut ti = TimeoutIterator::from_result_iterator(lines_iterator, 0);

        assert_eq!(ti.next().unwrap(), "1");
        assert_eq!(ti.next().unwrap(), "2");
        assert_eq!(ti.next().unwrap(), "3");
        assert_eq!(ti.next().unwrap(), "4");
        assert_eq!(ti.next().unwrap(), "5");
    }

    #[test]
    fn next_timeout() {
        let realistic_message = 
r"1
2
3
4
5";
        let lines_iterator = (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();

        let mut ti = TimeoutIterator::from_result_iterator(lines_iterator, 0);

        assert_eq!(ti.next().unwrap(), "1");
        assert_eq!(ti.next().unwrap(), "2");
        assert_eq!(ti.next().unwrap(), "3");
        assert_eq!(ti.next().unwrap(), "4");
        assert_eq!(ti.next().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }

        #[test]
    fn peek_doesnt_remove() {
        let realistic_message = 
r"1
2
3
4
5";
        let lines_iterator = (Box::new(realistic_message.as_bytes()) as Box<dyn BufRead + Send>).lines();
        let mut ti = TimeoutIterator::from_result_iterator(lines_iterator, 0);

        assert_eq!(ti.next().unwrap(), "1");
        assert_eq!(ti.next().unwrap(), "2");
        assert_eq!(ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), "3");
        assert_eq!(ti.next().unwrap(), "3");
        assert_eq!(ti.next().unwrap(), "4");
        assert_eq!(ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), "5");
        assert_eq!(ti.peek_timeout(Duration::from_secs(1)).ok().unwrap(), "5");
        assert_eq!(ti.next().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1));
        assert!(timeout_result.is_err());
    }


    #[test]
    fn item_iterator() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];
        let mut ti = TimeoutIterator::from_item_iterator(numbers.into_iter(), 0);

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
        let mut ti = TimeoutIterator::from_item_iterator(numbers.into_iter(), 0);
        thread::spawn(move || {
            ti.next();
        });
        assert!(true, "If this compiles, TimeoutIterator is Send'able across threads.");
    }
}


