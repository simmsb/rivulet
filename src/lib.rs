#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(rustdoc::broken_intra_doc_links, missing_docs)]
//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

mod base;
pub use base::*;

pub mod circular_buffer;
pub mod error;
pub mod io;
pub mod lazy;
pub mod slice;
pub mod splittable;

pub use circular_buffer::circular_buffer;
pub use splittable::Splittable;
