//! Offline ISO 15765-2:2016 (ISO-TP / `DoCAN`) transport/network-layer codec.
//!
//! `isotpkit` is the transport/network layer of ISO 15765-2:2016 (`DoCAN`): it segments a
//! UDS/GMLAN application message into CAN frames and reassembles it. It is the layer *under*
//! an application-layer UDS codec such as the sibling `udsdiag`, and it does not depend on one.
//!
//! The crate is *sans-I/O*: it has no bus, no sockets and no live timer loop. It encodes and
//! decodes the four N_PDU types, runs the segmentation/reassembly state machine, and names the
//! network-layer timing parameters as constants. A `FlowControl`-paced sender/receiver and a
//! live timer loop are deliberately out of scope (see `ROADMAP.md`).
//!
//! Every frame-format and parameter detail is grounded in ISO 15765-2:2016, read through the
//! `primarysource` `iso_*` tools; `docs/iso-map.md` is the clause-by-clause map.
//!
//! # Scope
//!
//! Classic CAN (`CAN_DL` at most 8 bytes) and normal addressing only. CAN FD, the `SingleFrame`
//! and `FirstFrame` escape sequences, and extended/mixed addressing are planned rows that the
//! decoder rejects rather than guessing.
//!
//! # Example
//!
//! ```
//! use isotpkit::segment::{segment, reassemble};
//!
//! let message = b"\x22\xf1\x90hello world"; // 14 bytes: needs a FirstFrame + ConsecutiveFrames
//! let pdus = segment(message).unwrap();
//! assert_eq!(reassemble(&pdus).unwrap(), message);
//! ```
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod addressing;
pub mod cli;
pub mod flowcontrol;
pub mod pci;
pub mod segment;
pub mod timing;

mod error;

pub use error::Error;

/// The name of this crate, as Cargo knows it.
#[must_use]
pub const fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}
