pub mod auto {
    pub async fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(a: A, b: B) {
        a.await;
        b.await
    }
}

pub mod manual {
    use core::{
        mem::ManuallyDrop,
        pin::Pin,
        task::{Context, Poll, ready},
    };

    use pin_project::{pin_project, pinned_drop};

    pub fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(
        a: A,
        b: B,
    ) -> impl Future<Output = ()> {
        DoAThenB::new(a, b)
    }

    #[pin_project(project = DoAThenBProj, PinnedDrop)]
    enum DoAThenB<A, B> {
        DoingA {
            #[pin]
            a: A,
            b: ManuallyDrop<B>,
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
                b: ManuallyDrop::new(b),
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
                        // SAFETY: we drop the `ManuallyDrop` right after
                        // taking from it, so `b` is only read once.
                        let b = unsafe { ManuallyDrop::take(b) };
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

    #[pinned_drop]
    impl<A, B> PinnedDrop for DoAThenB<A, B> {
        fn drop(self: Pin<&mut Self>) {
            let this = self.project();
            match this {
                // SAFETY: we immediately change states after taking from the
                // `ManuallyDrop`, so `b` must be initialized.
                DoAThenBProj::DoingA { a: _, b } => unsafe { ManuallyDrop::drop(b) },
                DoAThenBProj::DoingB { b: _ } => (),
                DoAThenBProj::Done => (),
            }
        }
    }
}

/// This version attempts to use tail calls to avoid looping in `poll`.
pub mod manual_opt {
    use core::{
        hint::unreachable_unchecked,
        mem::MaybeUninit,
        pin::Pin,
        task::{Context, Poll, ready},
    };

    use pin_project::{pin_project, pinned_drop};

    pub fn a_then_b<A: Future<Output = ()>, B: Future<Output = ()>>(
        a: A,
        b: B,
    ) -> impl Future<Output = ()> {
        DoAThenB::new(a, b)
    }

    #[pin_project(project = DoAThenBProj, PinnedDrop)]
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

    #[pinned_drop]
    impl<A, B> PinnedDrop for DoAThenB<A, B> {
        fn drop(self: Pin<&mut Self>) {
            let this = self.project();
            match this {
                // SAFETY: we immediately change states after reading from the
                // `MaybeUninit`, so `b` must be initialized.
                DoAThenBProj::DoingA { a: _, b } => unsafe { b.assume_init_drop() },
                // a is uninitialized in this state.
                DoAThenBProj::DoingB { a: _, b: _ } => (),
                DoAThenBProj::Done => (),
            }
        }
    }
}
