use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

pub struct PollableTarget {
    target: *mut (),
    poll: fn(*mut (), &mut Context<'_>) -> Poll<Option<&'static [u8]>>,
}

impl PollableTarget {
    pub fn from<F>(target: *mut F) -> Self
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        fn poll<F>(target: *mut (), cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>>
        where
            F: Future<Output = Option<&'static [u8]>>,
        {
            unsafe { Pin::new_unchecked(&mut *(target as *mut F)).poll(cx) }
        }

        Self {
            target: target as *mut (),
            poll: poll::<F>,
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>> {
        (self.poll)(self.target, cx)
    }
}
