use crate::error::Error;
use core::pin::Pin;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use pin_project::pin_project;
use std::time::Duration;
use tokio::time::timeout;
use tokio_stream::StreamExt;

#[pin_project]
pub struct TimeoutStream<R: Stream> {
    #[pin]
    source: R,
    buffer: Vec<R::Item>,
}

impl<R: Stream> TimeoutStream<R> {
    /**
     * Use this constructor
     */
    pub async fn with_stream(source: R) -> Result<TimeoutStream<R>, Error> {
        Ok(TimeoutStream {
            source,
            buffer: Vec::new(),
        })
    }

    pub async fn peek_timeout(self: Pin<&mut Self>, duration: Duration) -> Result<&R::Item, Error> {
        // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
        match timeout(duration, self.peek()).await {
            Ok(Some(item)) => Ok(item),
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::TimedOut),
        }
    }

    pub async fn next_timeout(
        mut self: Pin<&mut Self>,
        duration: Duration,
    ) -> Result<R::Item, Error> {
        // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
        match timeout(duration, self.next()).await {
            Ok(Some(item)) => Ok(item),
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::TimedOut),
        }
    }

    pub async fn peek(mut self: Pin<&mut Self>) -> Option<&R::Item> {
        if self.as_mut().project().buffer.is_empty() {
            match self.next().await {
                Some(item) => self.as_mut().project().buffer.push(item),
                None => return None,
            }
        }
        self.project().buffer.first()
    }
}

impl<R: Stream> Stream for TimeoutStream<R> {
    type Item = R::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<<Self as Stream>::Item>> {
        if !self.buffer.is_empty() {
            return Poll::Ready(Some(self.project().buffer.remove(0)));
        }

        self.project().source.poll_next(cx)
    }
}

#[cfg(all(test, feature = "async"))]
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
        let lines_iterator = iter((Box::new(realistic_message.as_bytes())).lines());

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
        let lines_iterator = iter((Box::new(realistic_message.as_bytes())).lines());

        let mut pinned_stream = Box::pin(TimeoutStream::with_stream(lines_iterator).await.unwrap());
        let mut ti = pinned_stream.as_mut();

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(ti.next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.next().await.unwrap().unwrap(), "4");
        assert_eq!(ti.next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.as_mut().next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn peek_timeout_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator = iter((Box::new(realistic_message.as_bytes())).lines());

        let mut ti = Box::pin(TimeoutStream::with_stream(lines_iterator).await.unwrap());

        assert_eq!(ti.next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.next().await.unwrap().unwrap(), "2");
        assert_eq!(
            ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "3"
        );
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "4");
        assert_eq!(
            ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(
            ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.as_mut().next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn peek_doesnt_remove() {
        let realistic_message = r"1
2
3
4
5";
        let lines_iterator = iter((Box::new(realistic_message.as_bytes())).lines());

        let mut ti = Box::pin(TimeoutStream::with_stream(lines_iterator).await.unwrap());

        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "1");
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "2");
        assert_eq!(ti.as_mut().peek().await.unwrap().as_ref().unwrap(), "3");
        assert_eq!(
            ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "3"
        );
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "3");
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "4");
        assert_eq!(
            ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap()
                .as_ref()
                .unwrap(),
            "5"
        );
        assert_eq!(ti.as_mut().peek().await.unwrap().as_ref().unwrap(), "5");
        assert_eq!(ti.as_mut().next().await.unwrap().unwrap(), "5");

        let timeout_result = ti.as_mut().next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }

    #[tokio::test]
    async fn item_iterator() {
        let numbers: Vec<u32> = vec![1, 2, 3, 4, 5];

        let mut ti = Box::pin(
            TimeoutStream::with_stream(iter(numbers.into_iter()))
                .await
                .unwrap(),
        );

        assert_eq!(ti.as_mut().next().await.unwrap(), 1);
        assert_eq!(ti.as_mut().next().await.unwrap(), 2);
        assert_eq!(
            *ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap(),
            3
        );
        assert_eq!(ti.as_mut().next().await.unwrap(), 3);
        assert_eq!(ti.as_mut().next().await.unwrap(), 4);
        assert_eq!(
            *ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap(),
            5
        );
        assert_eq!(
            *ti.as_mut()
                .peek_timeout(Duration::from_secs(1))
                .await
                .unwrap(),
            5
        );
        assert_eq!(ti.as_mut().next().await.unwrap(), 5);

        let timeout_result = ti.as_mut().next_timeout(Duration::from_secs(1)).await;
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

        let mut pinned_stream =
            Box::pin(TimeoutStream::with_stream(throttled_numbers).await.unwrap());
        let mut ti = pinned_stream.as_mut();

        assert_eq!(ti.as_mut().next().await.unwrap(), 1);
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.as_mut().next().await.unwrap(), 2);
        assert_matches!(
            ti.as_mut()
                .peek_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(*ti.as_mut().peek().await.unwrap(), 3);
        assert_eq!(ti.as_mut().next().await.unwrap(), 3);
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(500))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.as_mut().next().await.unwrap(), 4);
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_matches!(
            ti.as_mut()
                .next_timeout(Duration::from_millis(100))
                .await
                .unwrap_err(),
            Error::TimedOut
        );
        assert_eq!(ti.as_mut().next().await.unwrap(), 5);

        let timeout_result = ti.as_mut().next_timeout(Duration::from_secs(1)).await;
        assert!(timeout_result.is_err());
    }
}
