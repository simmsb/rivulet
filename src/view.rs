//! Views into asynchronous streams.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use pin_project::pin_project;

/// Future produced by [`View::grant`].
pub struct Grant<'a, T> {
    handle: &'a mut T,
    count: usize,
}

impl<'a, T> Future for Grant<'a, T>
where
    T: View,
{
    type Output = Result<(), T::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let count = self.count;
        Pin::new(&mut self.handle).poll_grant(cx, count)
    }
}

/// Obtain views into asynchronous contiguous-memory streams.
pub trait View: Sized + Unpin {
    /// The streamed type.
    type Item;

    /// The error produced by [`poll_grant`](`Self::poll_grant`).
    type Error: core::fmt::Debug;

    /// Obtain the current view of the stream.
    ///
    /// This view is obtained by successfully polling [`poll_grant`](`Self::poll_grant`) and
    /// advanced by calling [`release`](`Self::release`).
    ///
    /// If this slice is smaller than last successful grant request, the end of the stream has been
    /// reached and no additional values will be provided.
    fn view(&self) -> &[Self::Item];

    /// Attempt to obtain a view of at least `count` elements.
    ///
    /// If the request exceeds the maximum possible grant (if there is one), an error should be returned.
    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>>;

    /// Attempt to obtain a view of at least `count` elements.
    ///
    /// If the request exceeds the maximum possible grant (if there is one), an error should be returned.
    fn try_grant(
        self: &mut Self,
        count: usize,
    ) -> Result<bool, Self::Error>;

    /// Attempt to advance past the first `count` elements in the current view.
    ///
    /// # Panics
    /// If the request exceeds the current grant, this function should panic.
    fn release(&mut self, count: usize);

    /// Create a future that obtains a view of at least `count` elements.
    ///
    /// See [`poll_grant`](`Self::poll_grant`).
    fn grant(&mut self, count: usize) -> Grant<'_, Self> {
        Grant {
            handle: self,
            count,
        }
    }

    /// Obtains a view of at least `count` elements, blocking the current thread.
    ///
    /// See [`poll_grant`](`View::poll_grant`).
    fn blocking_grant(&mut self, count: usize) -> Result<(), Self::Error> {
        futures::executor::block_on(self.grant(count))
    }

    /// Maps this view to a new view producing error `E`.
    fn map_error<E, F>(self, f: F) -> MapError<Self, E, F>
    where
        F: Fn(Self::Error) -> E,
    {
        MapError {
            view: self,
            map: f,
            _error: core::marker::PhantomData,
        }
    }
}

impl<S: View> View for &mut S {
    type Item = S::Item;
    type Error = S::Error;

    fn view(&self) -> &[Self::Item] {
        View::view(*self)
    }

    fn poll_grant(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        S::poll_grant(Pin::new(&mut **self), cx, count)
    }

    fn try_grant(self: &mut Self, count: usize) -> Result<bool, Self::Error> {
        S::try_grant(&mut **self, count)
    }

    fn release(&mut self, count: usize) {
        S::release(self, count)
    }
}

/// Obtain mutable views into asynchronous contiguous-memory mutable streams.
pub trait ViewMut: View {
    /// Obtain the current mutable view of the stream.
    ///
    /// Identical semantics to [`view`](trait.View.html#tymethod.view), but returns a mutable
    /// slice.
    fn view_mut(&mut self) -> &mut [Self::Item];
}

impl<S: ViewMut> ViewMut for &mut S {
    fn view_mut(&mut self) -> &mut [Self::Item] {
        ViewMut::view_mut(*self)
    }
}

/// An error-mapped view produced by [`View::map_error`].
#[pin_project]
#[derive(Debug)]
pub struct MapError<V, E, F> {
    #[pin]
    view: V,
    map: F,
    _error: core::marker::PhantomData<E>,
}

impl<V, E, F> MapError<V, E, F> {
    /// Return the original view.
    pub fn into_inner(self) -> V {
        self.view
    }
}

impl<V, E, F> View for MapError<V, E, F>
where
    V: View,
    E: core::fmt::Debug,
    F: Fn(V::Error) -> E,
{
    type Item = V::Item;
    type Error = E;

    fn view(&self) -> &[Self::Item] {
        self.view.view()
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        let pinned = self.project();
        let f = pinned.map;
        pinned.view.poll_grant(cx, count).map(|r| r.map_err(f))
    }

    fn try_grant(self: &mut Self, count: usize) -> Result<bool, Self::Error> {
        self.view.try_grant(count).map_err(&self.map)
    }

    fn release(&mut self, count: usize) {
        self.view.release(count)
    }
}

impl<V, E, F> ViewMut for MapError<V, E, F>
where
    V: ViewMut,
    E: core::fmt::Debug,
    F: Fn(V::Error) -> E,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        self.view.view_mut()
    }
}

impl<V, E, F> Copy for MapError<V, E, F>
where
    V: Copy,
    F: Copy,
{
}

impl<V, E, F> Clone for MapError<V, E, F>
where
    V: Clone,
    F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            map: self.map.clone(),
            _error: core::marker::PhantomData,
        }
    }
}

impl<V, E, F> core::hash::Hash for MapError<V, E, F>
where
    V: core::hash::Hash,
    F: core::hash::Hash,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: core::hash::Hasher,
    {
        self.view.hash(state);
        self.map.hash(state);
    }
}
