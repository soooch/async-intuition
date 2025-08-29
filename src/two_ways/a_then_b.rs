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
    use std::task::ready;

    use pin_project::pin_project;

    pub async fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(a: A, b: B) {
        DoAThenB {
            state: State::PendingA(a),
            b,
        }
        .await
    }

    #[pin_project(project = StateProj)]
    enum State<A> {
        PendingA(#[pin] A),
        ReadyA,
    }

    #[pin_project]
    struct DoAThenB<A, B> {
        #[pin]
        state: State<A>,
        #[pin]
        b: B,
    }

    impl<A: Future<Output = ()>, B: Future<Output = ()>> Future for DoAThenB<A, B> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();
            loop {
                match this.state.as_mut().project() {
                    StateProj::PendingA(a) => {
                        ready!(a.poll(cx));
                        this.state.set(State::ReadyA);
                    }
                    StateProj::ReadyA => {
                        ready!(this.b.poll(cx));
                        break Poll::Ready(());
                    }
                }
            }
        }
    }
}
