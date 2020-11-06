//! Traits defining common stream interfaces.

use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// An error produced when polling a [`Sink`](trait.Sink.html) or [`Source`](trait.Source.html).
#[derive(Debug)]
pub enum Error {
    /// The stream is closed and cannot be accessed.
    Closed,

    /// The request is malformed and results in a buffer overflow.
    Overflow,

    /// Some other implementation-specific error.
    Other(Box<dyn std::error::Error + Send>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Closed => writeln!(f, "the stream has been closed"),
            Self::Overflow => writeln!(f, "buffer overflow"),
            Self::Other(err) => writeln!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(ref err) => err.source(),
            _ => None,
        }
    }
}

macro_rules! future {
    { $(#[$attr:meta])* $trait:ident => $type:ident => $poll:ident } => {
        $(#[$attr])*
        #[pin_project]
        pub struct $type<'a, T> {
            #[pin]
            handle: &'a mut T,
            count: usize,
        }

        impl<'a, T> Future for $type<'a, T>
        where
            T: $trait + Unpin,
        {
            type Output = Result<(), Error>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
                let count = self.count;
                let pinned = self.project();
                pinned.handle.$poll(cx, count)
            }
        }
    }
}

future! {
    /// Future produced by [`Sink::reserve`](trait.Sink.html#method.reserve)
    Sink => Reserve => poll_reserve
}
future! {
    /// Future produced by [`Sink::commit`](trait.Sink.html#method.commit)
    Sink => Commit => poll_commit
}
future! {
    /// Future produced by [`Source::request`](trait.Source.html#method.request)
    Source => Request => poll_request
}
future! {
    /// Future produced by [`Source::consume`](trait.Source.html#method.consume)
    Source => Consume => poll_consume
}

/// Interface for asynchronous contiguous-memory sinks.
///
/// Implementors of `Sink` may be called "writers".
pub trait Sink {
    /// The type to be written.
    type Item;

    /// The mutable buffer for writing data.
    ///
    /// The size of this buffer is determined by successfully polling
    /// [`poll_reserve`](trait.Sink.html#tymethod.poll_reserve), and isn't finalized until
    /// successfully polling [`poll_commit`](trait.Sink.html#tymethod.poll_commit).
    fn sink(&mut self) -> &mut [Self::Item];

    /// Attempt to reserve `count` elements in the writable buffer.
    fn poll_reserve(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    /// Attempt to commit the first `count` elements in the writable buffer to the stream.
    fn poll_commit(self: Pin<&mut Self>, cx: &mut Context, count: usize)
        -> Poll<Result<(), Error>>;

    /// Create a future that reserves `count` elements in the writable buffer.
    fn reserve<'a>(&'a mut self, count: usize) -> Reserve<'a, Self>
    where
        Self: Sized + Unpin,
    {
        Reserve {
            handle: self,
            count,
        }
    }

    /// Create a future that commits the first `count` elements in the writable buffer to the
    /// stream.
    fn commit<'a>(&'a mut self, count: usize) -> Commit<'a, Self>
    where
        Self: Sized + Unpin,
    {
        Commit {
            handle: self,
            count,
        }
    }
}

impl<S: ?Sized + Sink + Unpin> Sink for &mut S {
    type Item = S::Item;

    fn sink(&mut self) -> &mut [Self::Item] {
        Sink::sink(*self)
    }

    fn poll_reserve(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_reserve(Pin::new(&mut **self), cx, count)
    }

    fn poll_commit(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_commit(Pin::new(&mut **self), cx, count)
    }
}

/// Interface for asynchronous contiguous-memory sources.
///
/// Implementors of `Source` may be called "readers".
pub trait Source {
    /// The type to be read.
    type Item;

    /// The buffer for reading data.
    ///
    /// The size of this buffer is determined by successfully polling
    /// [`poll_request`](trait.Source.html#tymethod.poll_request), and is advanced by
    /// successfully polling [`poll_consume`](trait.Source.html#tymethod.poll_consume).
    fn source(&self) -> &[Self::Item];

    /// Attempt to read `count` elements into the buffer.
    fn poll_request(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    /// Attempt to advance past the first `count` elements in the buffer.
    fn poll_consume(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    /// Create a future that reads `count` elements into the buffer.
    fn request<'a>(&'a mut self, count: usize) -> Request<'a, Self>
    where
        Self: Sized + Unpin,
    {
        Request {
            handle: self,
            count,
        }
    }

    /// Create a future that advances past the first `count` elements in the buffer.
    fn consume<'a>(&'a mut self, count: usize) -> Consume<'a, Self>
    where
        Self: Sized + Unpin,
    {
        Consume {
            handle: self,
            count,
        }
    }
}

impl<S: ?Sized + Source + Unpin> Source for &mut S {
    type Item = S::Item;

    fn source(&self) -> &[Self::Item] {
        Source::source(*self)
    }

    fn poll_request(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_request(Pin::new(&mut **self), cx, count)
    }

    fn poll_consume(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_consume(Pin::new(&mut **self), cx, count)
    }
}

/// Interface for asynchronous contiguous-memory mutable sources.
pub trait SourceMut: Source {
    /// The mutable buffer for reading data.
    ///
    /// Identical semantics to [`source`](trait.Source.html#tymethod.source), but returns a mutable
    /// reference.
    fn source_mut(&mut self) -> &mut [Self::Item];
}

impl<S: ?Sized + SourceMut + Unpin> SourceMut for &mut S {
    fn source_mut(&mut self) -> &mut [Self::Item] {
        SourceMut::source_mut(*self)
    }
}
