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
        mem::MaybeUninit,
        pin::Pin,
        task::{Context, Poll, ready},
    };

    use pin_project::pin_project;

    pub fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(
        a: A,
        b: B,
    ) -> impl Future<Output = ()> {
        DoAThenB::new(a, b)
    }

    #[pin_project(project = DoAThenBProj)]
    enum DoAThenB<A, B> {
        DoingA {
            #[pin]
            a: A,
            b: MaybeUninit<B>,
        },
        DoingB {
            #[pin]
            b: B,
        },
        Done,
    }

    impl<A, B> DoAThenB<A, B> {
        pub fn new(a: A, b: B) -> Self {
            Self::DoingA {
                a,
                b: MaybeUninit::new(b),
            }
        }
    }

    impl<A: Future<Output = ()>, B: Future<Output = ()>> Future for DoAThenB<A, B> {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            loop {
                let this = self.as_mut().project();
                match this {
                    DoAThenBProj::DoingA { a, b } => {
                        ready!(a.poll(cx));
                        // SAFETY: b is initialized in `DoAThenB::new`. We read
                        // it only once then drop the `MaybeUninit` by setting
                        // the state to `DoingB`.
                        let b = unsafe { MaybeUninit::assume_init_read(b) };
                        self.set(DoAThenB::DoingB { b });
                    }
                    DoAThenBProj::DoingB { b } => {
                        ready!(b.poll(cx));
                        self.set(DoAThenB::Done);
                        break Poll::Ready(());
                    }
                    DoAThenBProj::Done => {
                        panic!("`async fn` resumed after completion");
                    }
                }
            }
        }
    }
}

/// This version attempts to use tail calls to avoid looping in `poll`.
pub mod manual_opt {
    use core::{
        future::Future,
        hint::unreachable_unchecked,
        mem::MaybeUninit,
        pin::Pin,
        task::{Context, Poll, ready},
    };

    use pin_project::pin_project;

    pub fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(
        a: A,
        b: B,
    ) -> impl Future<Output = ()> {
        DoAThenB::new(a, b)
    }

    #[pin_project(project = DoAThenBProj)]
    enum DoAThenB<A, B> {
        DoingA {
            #[pin]
            a: A,
            b: MaybeUninit<B>,
        },
        DoingB {
            a: MaybeUninit<A>,
            #[pin]
            b: B,
        },
        Done,
    }

    impl<A, B> DoAThenB<A, B> {
        pub fn new(a: A, b: B) -> Self {
            Self::DoingA {
                a,
                b: MaybeUninit::new(b),
            }
        }
    }

    impl<A: Future<Output = ()>, B: Future<Output = ()>> Future for DoAThenB<A, B> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match &*self {
                // SAFETY: `self` is in the `DoingA` state.
                DoAThenB::DoingA { .. } => unsafe { self.doing_a(cx) },
                // SAFETY: `self` is in the `DoingB` state.
                DoAThenB::DoingB { .. } => unsafe { self.doing_b(cx) },
                DoAThenB::Done => panic!("`async fn` resumed after completion"),
            }
        }
    }

    impl<A: Future<Output = ()>, B: Future<Output = ()>> DoAThenB<A, B> {
        /// # Safety
        ///
        /// `self` must be in the `DoingA` state.
        unsafe fn doing_a(mut self: Pin<&mut DoAThenB<A, B>>, cx: &mut Context<'_>) -> Poll<()> {
            // scope so temps are dropped before the tail call
            {
                let this = self.as_mut().project();
                let DoAThenBProj::DoingA { a, b } = this else {
                    // SAFETY: caller must ensure `self` is in the `DoingA` state.
                    unsafe { unreachable_unchecked() }
                };

                ready!(a.poll(cx));
                let a = MaybeUninit::uninit();
                // SAFETY: b is initialized in `DoAThenB::new`. We read
                // it only once then drop the `MaybeUninit` by setting
                // the state to `DoingB`.
                let b = unsafe { MaybeUninit::assume_init_read(b) };
                self.set(DoAThenB::DoingB { a, b });
            }

            // tail call hopefully
            // SAFETY: we've just set `self` to the `DoingB` state.
            unsafe { self.doing_b(cx) }
        }

        /// # Safety
        ///
        /// `self` must be in the `DoingB` state.
        unsafe fn doing_b(mut self: Pin<&mut DoAThenB<A, B>>, cx: &mut Context<'_>) -> Poll<()> {
            let this = self.as_mut().project();
            let DoAThenBProj::DoingB { a: _, b } = this else {
                // SAFETY: caller must ensure `self` is in the `DoingB` state.
                unsafe { unreachable_unchecked() }
            };

            ready!(b.poll(cx));
            self.set(DoAThenB::Done);
            Poll::Ready(())
        }
    }
}
