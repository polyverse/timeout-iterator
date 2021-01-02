use core::pin::Pin;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use std::time::Duration;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use crate::error::Error;

pub struct TimeoutStream<T, R>
where
    T: Unpin,
    R: Stream<Item = T> + Unpin,
{
    source: R,
    buffer: Vec<T>,
}

impl<T, R> TimeoutStream<T, R>
where
    T: Unpin,
    R: Stream<Item = T> + Unpin,
{
    /**
     * Use this constructor
     */
    pub async fn with_stream(source: R) -> Result<TimeoutStream<T, R>, Error> {
        Ok(TimeoutStream {
            source,
            buffer: Vec::new(),
        })
    }

    pub async fn peek_timeout(&mut self, duration: Duration) -> Result<&T, Error> {
        // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
        match timeout(duration, self.peek()).await {
            Ok(Some(item)) => Ok(item),
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::TimedOut),
        }
    }

    pub async fn next_timeout(&mut self, duration: Duration) -> Result<T, Error> {
        // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
        match timeout(duration, self.next()).await {
            Ok(Some(item)) => Ok(item),
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::TimedOut),
        }
    }

    pub async fn peek(&mut self) -> Option<&T> {
        if self.buffer.is_empty() {
            match self.next().await {
                Some(item) => self.buffer.push(item),
                None => return None,
            }
        }
        self.buffer.first()
    }
}

impl<T, R> Stream for TimeoutStream<T, R>
where
    T: Unpin,
    R: Stream<Item = T> + Unpin,
{
    type Item = T;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<<Self as Stream>::Item>> {
        if !self.buffer.is_empty() {
            return Poll::Ready(Some(self.buffer.remove(0)));
        }

        Pin::new(&mut self.as_mut().source).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use futures::stream::iter;
    use std::io::BufRead;

    #[tokio::test]
    async fn iterates() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            iter((Box::new(realistic_message.as_bytes()) as Box<dyn BufRead>).lines());

        let mut ti = TimeoutStream::with_stream(lines_iterator).await.unwrap();

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(ti.next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.next().await.unwrap().unwrap(), "4");
        assert_eq!(ti.next().await.unwrap().unwrap(), "5");
    }

    #[tokio::test]
    async fn next_timeout() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            iter((Box::new(realistic_message.as_bytes()) as Box<dyn BufRead>).lines());

        let mut ti = TimeoutStream::with_stream(lines_iterator).await.unwrap();

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(ti.next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.next().await.unwrap().unwrap(), "4");
        assert_eq!(ti.next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn peek_timeout_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            iter((Box::new(realistic_message.as_bytes()) as Box<dyn BufRead>).lines());

        let mut ti = TimeoutStream::with_stream(lines_iterator).await.unwrap();

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "3"
        );
        assert_eq!(ti.next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.next().await.unwrap().unwrap(), "4");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(ti.next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn peek_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator =
            iter((Box::new(realistic_message.as_bytes()) as Box<dyn BufRead>).lines());

        let mut ti = TimeoutStream::with_stream(lines_iterator).await.unwrap();

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(ti.peek().await.unwrap().as_ref().unwrap(), "3");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "3"
        );
        assert_eq!(ti.next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.next().await.unwrap().unwrap(), "4");
        assert_eq!(
            ti.peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(ti.peek().await.unwrap().as_ref().unwrap(), "5");
        assert_eq!(ti.next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn item_iterator() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];

        let mut ti = TimeoutStream::with_stream(iter(numbers.into_iter()))
            .await
            .unwrap();

        assert_eq!(ti.next().await.unwrap(), 1);
        assert_eq!(ti.next().await.unwrap(), 2);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).await.unwrap(), 3);
        assert_eq!(ti.next().await.unwrap(), 3);
        assert_eq!(ti.next().await.unwrap(), 4);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).await.unwrap(), 5);
        assert_eq!(*ti.peek_timeout(Duration::from_secs(1)).await.unwrap(), 5);
        assert_eq!(ti.next().await.unwrap(), 5);

        let timeout_result = ti.next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn timedout_future_doesnt_drop_item() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];

        let throttled_numbers = Box::pin(
            iter(numbers.into_iter())
                // item every second at most
                .throttle(Duration::from_secs(1)),
        );

        let mut ti = TimeoutStream::with_stream(throttled_numbers).await.unwrap();

        assert_eq!(ti.next().await.unwrap(), 1);
        assert_matches!(
            ti.next_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.next().await.unwrap(), 2);
        assert_matches!(
            ti.peek_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(*ti.peek().await.unwrap(), 3);
        assert_eq!(ti.next().await.unwrap(), 3);
        assert_matches!(
            ti.next_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.next().await.unwrap(), 4);
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.next().await.unwrap(), 5);

        let timeout_result = ti.next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }
}
