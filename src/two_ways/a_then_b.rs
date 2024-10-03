pub mod auto {
    use core::future::Future;

    pub async fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(a: A, b: B) {
        a.await;
        b.await
    }
}

pub mod manual {
    use core::{
        future::Future,
        pin::{pin, Pin},
        task::Poll,
    };

    use pin_project::pin_project;

    pub async fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(a: A, b: B) {
        let state = State::default();
        DoAThenB { state, a, b }.await
    }

    #[derive(Clone, Copy, Default)]
    enum State {
        #[default]
        DoingA,
        DoingB,
    }

    #[pin_project]
    struct DoAThenB<A, B> {
        state: State,
        #[pin]
        a: A,
        #[pin]
        b: B,
    }

    impl<A: Future<Output = ()>, B: Future<Output = ()>> Future for DoAThenB<A, B> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
            let this = self.project();
            match this.state {
                State::DoingA => {
                    if this.a.poll(cx).is_ready() {
                        // TODO: drop a?
                        *this.state = State::DoingB;
                    }
                    Poll::Pending
                }
                State::DoingB => this.b.poll(cx),
            }
        }
    }
}
