use ::core::future::Future;
use ::core::mem::swap;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

pub fn select<F1, F2>(f1: F1, f2: F2) -> Select<F1, F2>
where
    F1: Future,
    F2: Future,
{
    Select {
        f1: Some(f1),
        f2: Some(f2),
    }
}

pub struct Select<F1, F2>
where
    F1: Future,
    F2: Future,
{
    f1: Option<F1>,
    f2: Option<F2>,
}

pub enum SelectResult<F1, F2>
where
    F1: Future,
    F2: Future,
{
    Result1(F1::Output, F2),
    Result2(F2::Output, F1),
    Failed(),
}

impl<F1, F2> Future for Select<F1, F2>
where
    F1: Future + Unpin + 'static,
    F2: Future + Unpin + 'static,
{
    type Output = SelectResult<F1, F2>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f1 = match &mut self.f1 {
            Some(value) => unsafe { Pin::new_unchecked(value) },
            _ => return Poll::Ready(SelectResult::Failed()),
        };

        if let Poll::Ready(value) = f1.poll(cx) {
            let mut target = None;
            swap(&mut target, &mut self.f2);

            return match target {
                Some(other) => Poll::Ready(SelectResult::Result1(value, other)),
                None => Poll::Ready(SelectResult::Failed()),
            };
        }

        let f2 = match &mut self.f2 {
            Some(value) => unsafe { Pin::new_unchecked(value) },
            _ => return Poll::Ready(SelectResult::Failed()),
        };

        if let Poll::Ready(value) = f2.poll(cx) {
            let mut target = None;
            swap(&mut target, &mut self.f1);

            return match target {
                Some(other) => Poll::Ready(SelectResult::Result2(value, other)),
                None => Poll::Ready(SelectResult::Failed()),
            };
        }

        Poll::Pending
    }
}
