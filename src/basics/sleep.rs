use core::{
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};

use std::{
    sync::{Arc, Mutex},
    thread,
};

use pin_project::pin_project;

/// A very bad implementation of async sleep which spawns a thread.
///
/// A realistic implementation would register a waker with a reactor which
/// itself would use a timer wheel or similar data structure.
pub async fn sleep(duration: Duration) {
    Sleep {
        duration,
        handle: None,
    }
    .await
}

struct Shared {
    waker: Waker,
    done: bool,
}

type Handle = Arc<Mutex<Shared>>;

#[pin_project]
pub struct Sleep {
    duration: Duration,
    handle: Option<Handle>,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let duration = self.duration;
        // check out the pin_project docs for more info on this. check out the
        // pin_and_suffering module for more info on pinning in general.
        let this = self.project();

        // on first poll, associate the waker with a "reactor" (here we just
        // spawn a thread which sleeps then calls wake on the waker).
        let handle = this.handle.get_or_insert_with(|| {
            let waker = cx.waker().clone();
            let handle = Arc::new(Mutex::new(Shared { waker, done: false }));

            thread::spawn({
                let handle = Arc::clone(&handle);
                move || {
                    thread::sleep(duration);
                    let mut shared = handle.lock().unwrap();
                    shared.done = true;
                    shared.waker.wake_by_ref();
                }
            });

            handle
        });

        let mut shared = handle.lock().unwrap();

        // consider why we can't just hold onto the thread JoinHandle and check
        // `JoinHandle::is_finished` instead of maintaining our own `done` flag.
        if !shared.done {
            // we update the waker registered with the reactor in case the
            // executor that is polling us has changed.
            shared.waker.clone_from(cx.waker());
            return Poll::Pending;
        }

        Poll::Ready(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use futures_lite::future::block_on;

    use super::*;

    #[test]
    fn works() {
        const DURATION: Duration = Duration::from_millis(50);
        let start = Instant::now();
        block_on(sleep(DURATION));
        assert!(start.elapsed() >= DURATION);
    }
}
