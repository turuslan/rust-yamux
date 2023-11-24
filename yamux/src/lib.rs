// Copyright (c) 2018-2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 or MIT license, at your option.
//
// A copy of the Apache License, Version 2.0 is included in the software as
// LICENSE-APACHE and a copy of the MIT license is included in the software
// as LICENSE-MIT. You may also obtain a copy of the Apache License, Version 2.0
// at https://www.apache.org/licenses/LICENSE-2.0 and a copy of the MIT license
// at https://opensource.org/licenses/MIT.

//! This crate implements the [Yamux specification][1].
//!
//! It multiplexes independent I/O streams over reliable, ordered connections,
//! such as TCP/IP.
//!
//! The two primary objects, clients of this crate interact with, are:
//!
//! - [`Connection`], which wraps the underlying I/O resource, e.g. a socket, and
//!   provides methods for opening outbound or accepting inbound streams.
//! - [`Stream`], which implements [`futures::io::AsyncRead`] and
//!   [`futures::io::AsyncWrite`].
//!
//! [1]: https://github.com/hashicorp/yamux/blob/master/spec.md

#![forbid(unsafe_code)]

mod chunks;
mod error;
mod frame;

pub(crate) mod connection;
mod tagged_stream;

pub use crate::connection::{Connection, Mode, Packet, Stream};
pub use crate::error::ConnectionError;
pub use crate::frame::{
    header::{HeaderDecodeError, StreamId},
    FrameDecodeError,
};

pub const DEFAULT_CREDIT: usize = 256 * 1024; // as per yamux specification

pub type Result<T> = std::result::Result<T, ConnectionError>;

/// The maximum number of streams we will open without an acknowledgement from the other peer.
///
/// This enables a very basic form of backpressure on the creation of streams.
const MAX_ACK_BACKLOG: usize = 256;

/// Default maximum number of bytes a Yamux data frame might carry as its
/// payload when being send. Larger Payloads will be split.
///
/// The data frame payload size is not restricted by the yamux specification.
/// Still, this implementation restricts the size to:
///
/// 1. Reduce delays sending time-sensitive frames, e.g. window updates.
/// 2. Minimize head-of-line blocking across streams.
/// 3. Enable better interleaving of send and receive operations, as each is
///    carried out atomically instead of concurrently with its respective
///    counterpart.
///
/// For details on why this concrete value was chosen, see
/// https://github.com/paritytech/yamux/issues/100.
const DEFAULT_SPLIT_SEND_SIZE: usize = 16 * 1024;


// TODO: Update
/// Yamux configuration.
///
/// The default configuration values are as follows:
///
/// - receive window = 256 KiB
/// - max. buffer size (per stream) = 1 MiB
/// - max. number of streams = 8192
/// - window update mode = on read
/// - read after close = true
/// - split send size = 16 KiB
#[derive(Debug, Clone)]
pub struct Config {
    max_stream_receive_window: Option<usize>,
    max_connection_receive_window: usize,
    max_num_streams: usize,
    read_after_close: bool,
    split_send_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            // TODO: Add rational: given that we have a connection window, ...
            max_stream_receive_window: None,
            // TODO: reevaluate default.
            // TODO: Add setter.
            max_connection_receive_window: 1 * 1024 * 1024 * 1024,
            // TODO
            max_num_streams: 512,
            read_after_close: true,
            split_send_size: DEFAULT_SPLIT_SEND_SIZE,
        }
    }
}

impl Config {
    /// Set the receive window per stream (must be >= 256 KiB).
    ///
    /// # Panics
    ///
    /// If the given receive window is < 256 KiB.
    pub fn set_max_stream_receive_window(&mut self, n: Option<usize>) -> &mut Self {
        self.max_stream_receive_window = n;
        self.check();
        self
    }

    pub fn set_max_connection_receive_window(&mut self, n: usize) -> &mut Self {
        self.max_connection_receive_window = n;
        self.check();
        self
    }

    /// Set the max. number of streams.
    pub fn set_max_num_streams(&mut self, n: usize) -> &mut Self {
        self.max_num_streams = n;
        self.check();
        self
    }

    /// Allow or disallow streams to read from buffered data after
    /// the connection has been closed.
    pub fn set_read_after_close(&mut self, b: bool) -> &mut Self {
        self.read_after_close = b;
        self.check();
        self
    }

    /// Set the max. payload size used when sending data frames. Payloads larger
    /// than the configured max. will be split.
    pub fn set_split_send_size(&mut self, n: usize) -> &mut Self {
        self.split_send_size = n;
        self.check();
        self
    }

    // TODO: Consider doing the check on creation, not on each builder method call.
    fn check(&self) {
        assert!(self.max_stream_receive_window.unwrap_or(usize::MAX) >= DEFAULT_CREDIT);
        assert!(self.max_connection_receive_window >= self.max_num_streams * DEFAULT_CREDIT);
    }
}

// Check that we can safely cast a `usize` to a `u64`.
static_assertions::const_assert! {
    std::mem::size_of::<usize>() <= std::mem::size_of::<u64>()
}

// Check that we can safely cast a `u32` to a `usize`.
static_assertions::const_assert! {
    std::mem::size_of::<u32>() <= std::mem::size_of::<usize>()
}
