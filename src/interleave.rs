//! Interleaving: concurrent reassembly of messages on distinct `N_AI` (§9.9).
//!
//! §9.9 requires the network layer to carry out parallel transmission of different messages that are
//! not mapped onto the same `N_AI`, so a gateway can handle concurrent transmissions across distinct
//! sub-networks. §8.3.2.4 adds the isolation rule: unexpected-PDU error handling "only pertains to
//! messages with the same `N_AI`", so a frame on one channel must never disturb a reception on
//! another.
//!
//! [`Interleaver`] is the receive-side demultiplexer that provides this. It keeps one
//! [`RxSession`] per [`AddressInfo`] (`N_AI`) and routes each frame to its channel, so concurrent
//! segmented messages reassemble independently and the §9.8.3 unexpected-PDU handling stays scoped
//! to a single `N_AI`. It is still sans-I/O: it owns no bus and no timer. Concurrent *transmission*
//! needs no shared state — each message is an independent [`TxSession`](crate::session::TxSession).
//!
//! # Example
//!
//! ```
//! use isotpkit::pci::Addressing;
//! use isotpkit::segment::segment;
//! use isotpkit::service::{AddressInfo, TargetAddressType};
//! use isotpkit::interleave::Interleaver;
//! use isotpkit::session::RxAction;
//!
//! let ai_a = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
//! let ai_b = AddressInfo::diagnostics(0xF1, 0x11, TargetAddressType::Physical);
//! let mut mux = Interleaver::new(Addressing::Normal, 1 << 20, 0, 0, 4);
//!
//! let a = segment(&(0..16).collect::<Vec<u8>>()).unwrap();
//! // A FirstFrame on channel A opens a reception there without touching any other channel.
//! assert!(matches!(mux.on_frame(ai_a, &a[0].encode()).unwrap(), RxAction::SendFlowControl(_)));
//! // A SingleFrame on channel B completes on its own, leaving A's reception in progress.
//! assert!(matches!(mux.on_frame(ai_b, &[0x02, 0x3E, 0x00]).unwrap(), RxAction::Message(_)));
//! assert!(mux.in_progress(ai_a));
//! ```

use alloc::vec::Vec;

use crate::error::Error;
use crate::pci::Addressing;
use crate::service::AddressInfo;
use crate::session::{RxAction, RxSession};

struct Channel {
    ai: AddressInfo,
    session: RxSession,
}

/// A receive-side demultiplexer that reassembles messages on distinct `N_AI` concurrently (§9.9).
///
/// Every channel shares the receiver configuration the [`Interleaver`] is built with (addressing
/// format, buffer limit, advertised `BlockSize`/`STmin`/`N_WFTmax`); the channels differ only by
/// their `N_AI`. A channel is created on first use and dropped as soon as it carries no reception in
/// progress, so the interleaver holds exactly the `N_AI` that have a message in flight.
pub struct Interleaver {
    addressing: Addressing,
    buffer_limit: usize,
    block_size: u8,
    stmin: u8,
    wft_max: u8,
    channels: Vec<Channel>,
}

impl Interleaver {
    /// A demultiplexer whose per-`N_AI` receivers all use this addressing format and configuration
    /// (the same parameters as [`RxSession::new`]).
    #[must_use]
    pub const fn new(
        addressing: Addressing,
        buffer_limit: usize,
        block_size: u8,
        stmin: u8,
        wft_max: u8,
    ) -> Interleaver {
        Interleaver {
            addressing,
            buffer_limit,
            block_size,
            stmin,
            wft_max,
            channels: Vec::new(),
        }
    }

    /// Route one received frame to the receiver for its `N_AI`, creating that channel on first use.
    ///
    /// The returned [`RxAction`] is that channel's own, exactly as [`RxSession::on_frame`] would
    /// return it; frames on other `N_AI` are untouched. A channel left with no reception in progress
    /// (a completed, failed, or stray frame) is dropped before returning.
    ///
    /// # Errors
    /// Any [`Error`] [`RxSession::on_frame`] returns for the routed frame.
    pub fn on_frame(&mut self, ai: AddressInfo, frame: &[u8]) -> Result<RxAction, Error> {
        let index = if let Some(index) = self.channels.iter().position(|c| c.ai == ai) {
            index
        } else {
            self.channels.push(Channel {
                ai,
                session: RxSession::new(
                    self.addressing,
                    self.buffer_limit,
                    self.block_size,
                    self.stmin,
                    self.wft_max,
                ),
            });
            self.channels.len() - 1
        };
        let result = self.channels[index].session.on_frame(frame);
        // Keep only the channels that still hold a reception; an idle one has no state worth storing.
        if !self.channels[index].session.in_progress() {
            self.channels.remove(index);
        }
        result
    }

    /// Whether a segmented reception is in progress on the given `N_AI`.
    #[must_use]
    pub fn in_progress(&self, ai: AddressInfo) -> bool {
        self.channels
            .iter()
            .any(|c| c.ai == ai && c.session.in_progress())
    }

    /// How many `N_AI` currently have a reception in progress.
    #[must_use]
    pub fn active_receptions(&self) -> usize {
        self.channels
            .iter()
            .filter(|c| c.session.in_progress())
            .count()
    }

    /// Abandon the reception in progress on one `N_AI`, dropping its channel.
    pub fn reset(&mut self, ai: AddressInfo) {
        self.channels.retain(|c| c.ai != ai);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        reason = "test fixtures build byte patterns from loop indices"
    )]
    use super::*;
    use crate::segment::segment;
    use crate::service::TargetAddressType;
    use alloc::vec;
    use alloc::vec::Vec;

    fn ai(n_ta: u8) -> AddressInfo {
        AddressInfo::diagnostics(0xF1, n_ta, TargetAddressType::Physical)
    }

    #[test]
    fn messages_on_distinct_nai_reassemble_independently() {
        let msg_a: Vec<u8> = (0..16).collect();
        let msg_b: Vec<u8> = (0u8..16).map(|i| 100 + i).collect();
        let fa = segment(&msg_a).unwrap();
        let fb = segment(&msg_b).unwrap();
        let (ai_a, ai_b) = (ai(0x10), ai(0x11));
        let mut mux = Interleaver::new(Addressing::Normal, 1 << 20, 0, 0, 4);

        // Open both receptions, then interleave their ConsecutiveFrames frame-for-frame.
        assert!(matches!(
            mux.on_frame(ai_a, &fa[0].encode()).unwrap(),
            RxAction::SendFlowControl(_)
        ));
        assert!(matches!(
            mux.on_frame(ai_b, &fb[0].encode()).unwrap(),
            RxAction::SendFlowControl(_)
        ));
        assert_eq!(mux.active_receptions(), 2);
        assert_eq!(
            mux.on_frame(ai_a, &fa[1].encode()).unwrap(),
            RxAction::Continue
        );
        assert_eq!(
            mux.on_frame(ai_b, &fb[1].encode()).unwrap(),
            RxAction::Continue
        );
        assert_eq!(
            mux.on_frame(ai_a, &fa[2].encode()).unwrap(),
            RxAction::Message(msg_a)
        );
        assert_eq!(
            mux.on_frame(ai_b, &fb[2].encode()).unwrap(),
            RxAction::Message(msg_b)
        );
        // Both complete: their channels are pruned.
        assert_eq!(mux.active_receptions(), 0);
    }

    #[test]
    fn a_frame_on_a_second_nai_does_not_interrupt_the_first() {
        let msg: Vec<u8> = (0..16).collect();
        let frames = segment(&msg).unwrap();
        let (ai_a, ai_b) = (ai(0x10), ai(0x11));
        let mut mux = Interleaver::new(Addressing::Normal, 1 << 20, 0, 0, 4);

        // Open a segmented reception on channel A.
        assert!(matches!(
            mux.on_frame(ai_a, &frames[0].encode()).unwrap(),
            RxAction::SendFlowControl(_)
        ));
        // A SingleFrame on channel B must NOT be treated as an unexpected PDU for A (§8.3.2.4): it
        // completes on its own, and A's reception is untouched — contrast a single RxSession, where
        // this would raise N_UNEXP_PDU and restart.
        assert_eq!(
            mux.on_frame(ai_b, &[0x02, 0x3E, 0x00]).unwrap(),
            RxAction::Message(vec![0x3E, 0x00])
        );
        assert!(mux.in_progress(ai_a));
        assert_eq!(mux.active_receptions(), 1);

        // Channel A completes normally.
        assert_eq!(
            mux.on_frame(ai_a, &frames[1].encode()).unwrap(),
            RxAction::Continue
        );
        assert_eq!(
            mux.on_frame(ai_a, &frames[2].encode()).unwrap(),
            RxAction::Message((0..16).collect())
        );
        assert_eq!(mux.active_receptions(), 0);
    }

    #[test]
    fn idle_and_reset_channels_are_dropped() {
        let (ai_a, ai_b) = (ai(0x10), ai(0x11));
        let mut mux = Interleaver::new(Addressing::Normal, 1 << 20, 0, 0, 4);
        // A stray ConsecutiveFrame on a fresh N_AI is ignored and leaves no channel behind.
        assert_eq!(
            mux.on_frame(ai_a, &[0x21, 1, 2, 3]).unwrap(),
            RxAction::Ignored
        );
        assert_eq!(mux.active_receptions(), 0);

        // Open a reception on B, then abandon it with reset.
        let frames = segment(&(0..16).collect::<Vec<u8>>()).unwrap();
        mux.on_frame(ai_b, &frames[0].encode()).unwrap();
        assert!(mux.in_progress(ai_b));
        mux.reset(ai_b);
        assert!(!mux.in_progress(ai_b));
        assert_eq!(mux.active_receptions(), 0);
    }
}
