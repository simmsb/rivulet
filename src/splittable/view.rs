use super::{SplittableView, SplittableViewMut};
use futures::task::AtomicWaker;
use std::{
    convert::TryInto,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// A view returned by [`SplittableView::into_view`](`super::SplittableView::into_view`).
pub struct View<T>
where
    T: SplittableView,
{
    splittable: T,
    waker: Arc<AtomicWaker>,
    head: u64,
    len: usize,
}

impl<T> View<T>
where
    T: SplittableView,
{
    pub(crate) fn new(splittable: T) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        // Safety: we have unique ownership of this
        unsafe {
            let waker = waker.clone();
            splittable.set_reader_waker(move || waker.wake());
        }
        Self {
            splittable,
            waker,
            head: 0,
            len: 0,
        }
    }
}

impl<T> crate::View for View<T>
where
    T: SplittableView,
{
    type Item = T::Item;
    type Error = T::Error;

    fn view(&self) -> &[Self::Item] {
        // Safety: we have unique ownership of the view, so this doesn't overlap with any other views
        unsafe { self.splittable.view(self.head, self.len) }
    }

    fn poll_grant(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        match Pin::new(&self.splittable).poll_available(
            cx,
            |waker| self.waker.register(waker),
            self.head,
            count,
        ) {
            Poll::Ready(Ok(len)) => {
                self.len = len;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn try_grant(self: &mut Self, count: usize) -> Result<bool, Self::Error> {
        match self.splittable.try_available(self.head, count) {
            Ok(0) => Ok(false),
            Ok(len) => {
                self.len = len;
                Ok(true)
            },
            Err(e) => Err(e)
        }
    }

    fn release(&mut self, count: usize) {
        assert!(
            count <= self.len,
            "attempted to release more than current grant"
        );

        self.len -= count;
        let count: u64 = count.try_into().unwrap();
        self.head += count;
        // Safety: we never read earlier than this head value
        unsafe {
            self.splittable.set_head(self.head);
        }
    }
}

impl<T> crate::ViewMut for View<T>
where
    T: SplittableViewMut,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        // Safety: we have unique ownership of the view, so this doesn't overlap with any other views
        unsafe { self.splittable.view_mut(self.head, self.len) }
    }
}
