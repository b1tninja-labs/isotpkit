//! Offline ISO 15765-2:2016 (ISO-TP / `DoCAN`) transport/network-layer codec.
//!
//! `isotpkit` is the transport/network layer of ISO 15765-2:2016 (`DoCAN`): it segments a
//! UDS/GMLAN application message into CAN frames and reassembles it. It is the layer *under*
//! an application-layer UDS codec such as the sibling `udskit`, and it does not depend on one.
//!
//! The crate is *sans-I/O*: it has no bus, no sockets and no live timer loop. It encodes and
//! decodes the four N_PDU types, runs the segmentation/reassembly state machine, names the
//! network-layer timing parameters as constants, and paces a sender/receiver in the [`session`]
//! module by *emitting* actions (send this frame, wait for a `FlowControl`, wait this `STmin`)
//! rather than running timers. A live timer loop is out of scope (see `ROADMAP.md`).
//!
//! Every frame-format and parameter detail is grounded in ISO 15765-2:2016, read through the
//! `primarysource` `iso_*` tools; `docs/iso-map.md` is the clause-by-clause map.
//!
//! # Scope
//!
//! Classic CAN (`CAN_DL` 8) and CAN FD (`CAN_DL` up to 64) frames, normal addressing — including
//! the `SingleFrame` escape (`SF_DL` > 7, in N_PCI byte #2) and the 32-bit `FirstFrame` escape
//! (`FF_DL` > 4095). Extended/mixed addressing, a `FlowControl`-paced loop and a live timer loop
//! are planned rows (see `ROADMAP.md`); the decoder rejects frames it does not implement rather
//! than guessing.
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
pub mod result;
pub mod segment;
pub mod service;
pub mod session;
pub mod timing;

mod error;

pub use error::Error;
pub use result::NResult;

/// The name of this crate, as Cargo knows it.
#[must_use]
pub const fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}
