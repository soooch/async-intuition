pub mod auto {
    use core::future::Future;

    pub async fn until_equals<NumFut, GetNextFut>(check: u32, next: GetNextFut)
    where
        NumFut: Future<Output = u32>,
        GetNextFut: Fn() -> NumFut,
    {
        loop {
            let num = next().await;
            if num == check {
                return;
            }
        }
    }
}

pub mod manual {
    use core::{
        future::Future,
        pin::{Pin, pin},
        task::Poll,
    };

    use pin_project::pin_project;

    pub async fn until_equals<NumFut, GetNextFut>(check: u32, next: GetNextFut)
    where
        NumFut: Future<Output = u32>,
        GetNextFut: Fn() -> NumFut,
    {
        UntilEquals {
            check,
            next,
            num_fut: None,
        }
        .await
    }

    #[pin_project]
    struct UntilEquals<NumFut, GetNextFut> {
        check: u32,
        next: GetNextFut,
        #[pin]
        num_fut: Option<NumFut>,
    }

    impl<NumFut, GetNextFut> Future for UntilEquals<NumFut, GetNextFut>
    where
        NumFut: Future<Output = u32>,
        GetNextFut: Fn() -> NumFut,
    {
        type Output = ();
        fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();

            let num_fut = if let Some(fut) = this.num_fut.as_mut().as_pin_mut() {
                fut
            } else {
                this.num_fut.set(Some((this.next)()));
                unsafe { this.num_fut.as_mut().as_pin_mut().unwrap_unchecked() }
            };

            match num_fut.poll(cx) {
                Poll::Ready(num) => {
                    this.num_fut.set(None);
                    if num == *this.check {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }
}
