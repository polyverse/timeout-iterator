[![Build Status](https://travis-ci.org/polyverse/timeout-iterator.svg?branch=master)](https://travis-ci.org/polyverse/timeout-iterator)

# timeout-iterator

`timeout_iterator::TimeoutIterator` is a wrapper over any iterator that adds two additional functions:
* peek_timeout()
* next_timeout()

The canonical use-case is parsing multi-line free-form records (such as tailing a log fime) where it is desirable to
consume the very last line, and peek whether the record continues on the next time, without blocking indefinitely on the peek().

This was built for parsing Kernel logs from `/dev/kmsg` for instance. A kernel log record may look like this:

```text
6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
 SUBSYSTEM=pci
 DEVICE=+pci:0000:00:05.0
 ```
The end of such a record is only known when the next line begins a new record. However, if this were the last
record emitted, then it is possible that it never gets flushed/parsed because there is no next record to trigger it.

This is where an iterator with a timeout capability helps to break the deadlock.

# Basic Iteration

There are two primary ways of constructing and using the iterator.

## The Item Iterator

The item iterator is obvious and intuitive:

```
use timeout_iterator::TimeoutIterator;

let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];
let mut ti = TimeoutIterator::from_item_iterator(numbers.into_iter()).unwrap();
```

This will iterate over all the integers in the Vector.

## The Result Iterator

Occasionally, your downstream iterator returns a Result<Item, _>, such as when
we read lines from a BufRead and we have to unwrap twice:

```
use timeout_iterator::TimeoutIterator;
use std::io::prelude::BufRead;

let logmessage =
r"6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
SUBSYSTEM=pci
DEVICE=+pci:0000:00:05.0";

let mut lines_iterator = (Box::new(logmessage.as_bytes()) as Box<dyn BufRead + Send>).lines();
let result = lines_iterator.next()
    .unwrap() // open the timeout iterator option
    .ok() // open the line-iterator result
    .unwrap(); // open the line-iterator option

assert_eq!(result, "6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode");
```

This can get worse when using *_timeout() functions, because they return a Result<> themselves, which
then further wraps another result. An item iterator from a timeout would unpack something like this:

```
use timeout_iterator::TimeoutIterator;
use std::io::prelude::BufRead;
use std::time::Duration;

let logmessage =
r"6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
SUBSYSTEM=pci
DEVICE=+pci:0000:00:05.0";

let mut lines_iterator = (Box::new(logmessage.as_bytes()) as Box<dyn BufRead + Send>).lines();
let mut ti = TimeoutIterator::from_item_iterator(lines_iterator).unwrap();
let result = ti.next_timeout(Duration::from_secs(1))
    .ok() // open the timeout iterator result
    .unwrap() // open the timeout iterator option
    .ok() // open the line-iterator result
    .unwrap(); // open the line-iterator option

assert_eq!(result, "6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode");
```

Knowing that the inner iterator provides `Result<Option<Foo>>`, we can tell the TimeoutIterator to
flatten it all into this:
```
use timeout_iterator::TimeoutIterator;
use std::io::prelude::BufRead;

let logmessage =
r"6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
SUBSYSTEM=pci
DEVICE=+pci:0000:00:05.0";

let mut lines_iterator = (Box::new(logmessage.as_bytes()) as Box<dyn BufRead + Send>).lines();
let mut ti = TimeoutIterator::from_result_iterator(lines_iterator).unwrap();

let result = ti.next()
    .unwrap(); // single unwrap of Option

assert_eq!(result, "6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode");
```


# Effectively using Timeouts

Now let's look at two log "Records", split over five lines:

```ignore
6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
 SUBSYSTEM=pci
 DEVICE=+pci:0000:00:05.0
6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
 DEVICE=+pci:0000:00:05.0
```

 We have an iterator over these lines, and we want to flush the second record cleanly, if no new
 lines are found within the next second (at that point, any future lines are for the next record).

 ```
use timeout_iterator::TimeoutIterator;
use std::io::prelude::BufRead;
use std::time::Duration;

let logmessage =
r"6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
  SUBSYSTEM=pci
  DEVICE=+pci:0000:00:05.0
6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode
  DEVICE=+pci:0000:00:05.0";

let mut lines_iterator = (Box::new(logmessage.as_bytes()) as Box<dyn BufRead + Send>).lines();
let mut ti = TimeoutIterator::from_result_iterator(lines_iterator).unwrap();

let mut records: Vec<String> = vec![];

loop {
    match ti.next_timeout(Duration::from_secs(1)) {
        Ok(next_line) => {
            let mut record = String::from(next_line);
            loop {
                match ti.peek_timeout(Duration::from_secs(1)) {
                    Ok(maybe_continuation_line) => {
                        if maybe_continuation_line.starts_with(' ') {
                            record.push('\n');
                            record.push_str(maybe_continuation_line);
                            ti.next(); //flush it
                        } else {
                            break;
                        }
                    },
                    Err(_) => break
                }
            }
            records.push(record);
        },
        Err(_) => break
    }
}

assert_eq!(records.remove(0), "6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode\n SUBSYSTEM=pci\n DEVICE=+pci:0000:00:05.0");

assert_eq!(records.remove(0), "6,361,518496,-;ahci 0000:00:05.0: AHCI 0001.0300 32 slots 6 ports 6 Gbps 0x1 impl SATA mode\n DEVICE=+pci:0000:00:05.0");

```