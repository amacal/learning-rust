use ::core::future::*;
use ::core::marker::*;
use ::core::mem;
use ::core::pin::*;
use ::core::task::*;

pub fn select<'a, F1, F2>(f1: F1, f2: F2) -> Select<'a, F1, F2>
where
    F1: Future + 'a,
    F2: Future + 'a,
{
    Select {
        f1: Some(f1),
        f2: Some(f2),
        _p: PhantomData,
    }
}

pub struct Select<'a, F1, F2>
where
    F1: Future,
    F2: Future,
{
    f1: Option<F1>,
    f2: Option<F2>,
    _p: PhantomData<&'a ()>,
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

impl<'a, F1, F2> Future for Select<'a, F1, F2>
where
    F1: Future + Unpin + 'a,
    F2: Future + Unpin + 'a,
{
    type Output = SelectResult<F1, F2>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f1 = match &mut self.f1 {
            Some(value) => unsafe { Pin::new_unchecked(value) },
            _ => return Poll::Ready(SelectResult::Failed()),
        };

        if let Poll::Ready(value) = f1.poll(cx) {
            let mut target = None;
            mem::swap(&mut target, &mut self.f2);

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
            mem::swap(&mut target, &mut self.f1);

            return match target {
                Some(other) => Poll::Ready(SelectResult::Result2(value, other)),
                None => Poll::Ready(SelectResult::Failed()),
            };
        }

        Poll::Pending
    }
}
