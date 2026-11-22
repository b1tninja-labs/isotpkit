//! A sans-I/O `FlowControl`-paced sender and receiver (ISO 15765-2:2016, §9.6.5, §9.7, §9.8.4).
//!
//! There is still no bus and no live timer here. Each session is a state machine that *emits*
//! actions — send this frame, wait for a `FlowControl`, wait this `SeparationTime` — which the
//! caller performs against its own bus and timers. That keeps the pacing logic (BlockSize blocks,
//! STmin spacing, FC.WAIT and FC.Overflow, `N_WFTmax`) testable without any I/O.
//!
//! # Example
//!
//! ```
//! use isotpkit::pci::{Addressing, FlowControl};
//! use isotpkit::session::{TxSession, TxAction};
//!
//! let mut tx = TxSession::new(&[0x22, 0xf1, 0x90, 1, 2, 3, 4, 5, 6, 7], 8, Addressing::Normal).unwrap();
//! assert!(matches!(tx.poll(), TxAction::SendFrame(_)));       // FirstFrame
//! assert_eq!(tx.poll(), TxAction::WaitForFlowControl);
//! tx.on_flow_control(FlowControl::continue_to_send());        // BlockSize 0 = send all
//! assert!(matches!(tx.poll(), TxAction::SendFrame(_)));       // ConsecutiveFrame
//! ```

use alloc::vec::Vec;

use crate::error::Error;
use crate::flowcontrol::{FlowStatus, SeparationTime};
use crate::pci::{Addressing, DEFAULT_PADDING, FlowControl, NPdu, decode_addressed};
use crate::segment::{Reassembler, SegmentPdu, segment_addressed};

/// What the sender should do next (the caller performs it, sans-I/O).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxAction {
    /// Transmit this already-encoded frame on the bus.
    SendFrame(Vec<u8>),
    /// Wait for a `FlowControl` frame from the receiver, then call [`TxSession::on_flow_control`].
    WaitForFlowControl,
    /// Wait at least this separation time before the next `ConsecutiveFrame` (§9.6.5).
    WaitSeparationTime(SeparationTime),
    /// The whole message has been sent.
    Done,
    /// The transfer was aborted; the error says why (e.g. a `FlowControl` Overflow).
    Aborted(Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TxState {
    SendFirstFrame,
    AwaitFlowControl,
    SendConsecutive,
    Separation,
    Done,
    Aborted,
}

/// A `FlowControl`-paced sender for one segmented message.
#[derive(Clone, Debug)]
pub struct TxSession {
    frames: Vec<SegmentPdu>,
    next: usize,
    addressing: Addressing,
    block_size: u8,
    stmin: SeparationTime,
    cfs_in_block: u8,
    state: TxState,
    abort: Option<Error>,
}

impl TxSession {
    /// Start a paced send of `message` for the given `TX_DL` and addressing format.
    ///
    /// # Errors
    /// Any [`Error`] [`segment_addressed`] returns (empty message, bad `tx_dl`, too long).
    pub fn new(message: &[u8], tx_dl: usize, addressing: Addressing) -> Result<TxSession, Error> {
        let frames = segment_addressed(message, tx_dl, addressing)?;
        Ok(TxSession {
            frames,
            next: 0,
            addressing,
            block_size: 0,
            stmin: SeparationTime::from_byte(0),
            cfs_in_block: 0,
            state: TxState::SendFirstFrame,
            abort: None,
        })
    }

    /// The next action for the caller to perform.
    pub fn poll(&mut self) -> TxAction {
        match self.state {
            TxState::SendFirstFrame => {
                let frame = self.frames[0].encode_addressed(self.addressing, DEFAULT_PADDING);
                self.next = 1;
                self.state = if self.frames.len() == 1 {
                    TxState::Done
                } else {
                    TxState::AwaitFlowControl
                };
                TxAction::SendFrame(frame)
            }
            TxState::AwaitFlowControl => TxAction::WaitForFlowControl,
            TxState::SendConsecutive => {
                if self.next >= self.frames.len() {
                    self.state = TxState::Done;
                    return TxAction::Done;
                }
                let frame =
                    self.frames[self.next].encode_addressed(self.addressing, DEFAULT_PADDING);
                self.next += 1;
                self.cfs_in_block += 1;
                self.state = if self.next >= self.frames.len() {
                    TxState::Done
                } else if self.block_size != 0 && self.cfs_in_block >= self.block_size {
                    TxState::AwaitFlowControl
                } else {
                    TxState::Separation
                };
                TxAction::SendFrame(frame)
            }
            TxState::Separation => {
                self.state = TxState::SendConsecutive;
                TxAction::WaitSeparationTime(self.stmin)
            }
            TxState::Done => TxAction::Done,
            TxState::Aborted => {
                TxAction::Aborted(self.abort.clone().unwrap_or(Error::IncompleteSequence))
            }
        }
    }

    /// Feed a received `FlowControl` frame while the sender is awaiting one.
    ///
    /// `ContinueToSend` adopts the frame's `BlockSize`/`STmin` and resumes sending; `Wait` keeps
    /// waiting (the caller restarts its `N_Bs` timer); `Overflow` aborts the transfer. A frame
    /// arriving when the sender is not awaiting one is ignored (§9.8.3).
    pub fn on_flow_control(&mut self, fc: FlowControl) {
        if self.state != TxState::AwaitFlowControl {
            return;
        }
        match fc.flow_status {
            FlowStatus::ContinueToSend => {
                self.block_size = fc.block_size;
                self.stmin = SeparationTime::from_byte(fc.stmin);
                self.cfs_in_block = 0;
                self.state = TxState::SendConsecutive;
            }
            FlowStatus::Wait => {}
            FlowStatus::Overflow => {
                self.abort = Some(Error::BufferOverflow);
                self.state = TxState::Aborted;
            }
        }
    }

    /// Whether the send has finished (every frame sent) or aborted.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(self.state, TxState::Done | TxState::Aborted)
    }
}

/// What the receiver should do after a frame (the caller performs it, sans-I/O).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RxAction {
    /// Send this `FlowControl` frame to the sender.
    SendFlowControl(FlowControl),
    /// A whole message has been reassembled.
    Message(Vec<u8>),
    /// More `ConsecutiveFrames` are expected; nothing to send yet.
    Continue,
}

/// A `FlowControl`-paced receiver. It accepts frames, reassembles the message, and decides when to
/// send a `FlowControl` (after the `FirstFrame` and after each block of `BlockSize` frames).
#[derive(Clone, Debug)]
pub struct RxSession {
    reassembler: Reassembler,
    addressing: Addressing,
    buffer_limit: usize,
    block_size: u8,
    stmin: u8,
    cfs_since_fc: u8,
    waits_sent: u8,
    wft_max: u8,
    active: bool,
}

impl RxSession {
    /// A receiver for the given addressing, buffer limit, and the `BlockSize`/`STmin`/`N_WFTmax` it
    /// advertises. `buffer_limit` is the largest message it will accept before answering Overflow.
    #[must_use]
    pub fn new(
        addressing: Addressing,
        buffer_limit: usize,
        block_size: u8,
        stmin: u8,
        wft_max: u8,
    ) -> RxSession {
        RxSession {
            reassembler: Reassembler::new(),
            addressing,
            buffer_limit,
            block_size,
            stmin,
            cfs_since_fc: 0,
            waits_sent: 0,
            wft_max,
            active: false,
        }
    }

    fn cts(&self) -> FlowControl {
        FlowControl::new(FlowStatus::ContinueToSend, self.block_size, self.stmin)
    }

    /// Feed one received frame.
    ///
    /// # Errors
    /// Any [`Error`] the decoder or reassembler returns, including [`Error::WrongSequenceNumber`]
    /// and [`Error::ConsecutiveFrameWithoutFirstFrame`].
    pub fn on_frame(&mut self, frame: &[u8]) -> Result<RxAction, Error> {
        match decode_addressed(frame, self.addressing)? {
            NPdu::Single(sf) => {
                self.reset();
                Ok(RxAction::Message(sf.data().to_vec()))
            }
            NPdu::First(ff) => {
                self.reset();
                if ff.length() > self.buffer_limit {
                    Ok(RxAction::SendFlowControl(FlowControl::new(
                        FlowStatus::Overflow,
                        0,
                        0,
                    )))
                } else {
                    self.reassembler.feed(&SegmentPdu::First(ff))?;
                    self.active = true;
                    self.cfs_since_fc = 0;
                    self.waits_sent = 0;
                    Ok(RxAction::SendFlowControl(self.cts()))
                }
            }
            NPdu::Consecutive(cf) => {
                if !self.active {
                    return Err(Error::ConsecutiveFrameWithoutFirstFrame);
                }
                if let Some(message) = self.reassembler.feed(&SegmentPdu::Consecutive(cf))? {
                    self.reset();
                    return Ok(RxAction::Message(message));
                }
                self.cfs_since_fc += 1;
                if self.block_size != 0 && self.cfs_since_fc >= self.block_size {
                    self.cfs_since_fc = 0;
                    self.waits_sent = 0;
                    Ok(RxAction::SendFlowControl(self.cts()))
                } else {
                    Ok(RxAction::Continue)
                }
            }
            // A receiver does not expect a FlowControl; ignore it (§9.8.3).
            NPdu::Flow(_) => Ok(RxAction::Continue),
        }
    }

    /// Ask to send an FC.WAIT because the upper layer is not ready for the next block.
    ///
    /// # Errors
    /// [`Error::WaitFrameOverrun`] once `N_WFTmax` WAIT frames have been sent in a row (`N_WFT_OVRN`,
    /// §9.8.4); the caller must then abort the reception.
    pub fn request_wait(&mut self) -> Result<RxAction, Error> {
        if self.waits_sent >= self.wft_max {
            self.reset();
            return Err(Error::WaitFrameOverrun(self.wft_max));
        }
        self.waits_sent += 1;
        Ok(RxAction::SendFlowControl(FlowControl::new(
            FlowStatus::Wait,
            0,
            0,
        )))
    }

    /// Whether a segmented message is being received.
    #[must_use]
    pub const fn in_progress(&self) -> bool {
        self.active
    }

    fn reset(&mut self) {
        self.reassembler.reset();
        self.active = false;
        self.cfs_since_fc = 0;
        self.waits_sent = 0;
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        reason = "test fixtures build byte patterns from loop indices"
    )]
    use super::*;
    use alloc::vec;

    /// Drive a sender and receiver against each other and return the reassembled message.
    fn run(
        message: &[u8],
        tx_dl: usize,
        addressing: Addressing,
        block_size: u8,
        stmin: u8,
    ) -> Vec<u8> {
        let mut tx = TxSession::new(message, tx_dl, addressing).unwrap();
        let mut rx = RxSession::new(addressing, 1 << 20, block_size, stmin, 4);
        let mut received = None;
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 100_000, "runaway pacing loop");
            match tx.poll() {
                TxAction::SendFrame(frame) => match rx.on_frame(&frame).unwrap() {
                    RxAction::SendFlowControl(fc) => tx.on_flow_control(fc),
                    RxAction::Message(m) => received = Some(m),
                    RxAction::Continue => {}
                },
                // The receiver's FlowControl was delivered inline above; nothing to wait on here.
                TxAction::WaitForFlowControl | TxAction::WaitSeparationTime(_) => {}
                TxAction::Done => break,
                TxAction::Aborted(e) => panic!("unexpected abort: {e}"),
            }
        }
        received.expect("message not received")
    }

    #[test]
    fn paced_transfer_round_trips_across_block_sizes() {
        for &bs in &[0u8, 1, 3, 8] {
            for len in [10usize, 100, 300] {
                let message: Vec<u8> = (0..len).map(|i| (i * 7 + 1) as u8).collect();
                assert_eq!(
                    run(&message, 8, Addressing::Normal, bs, 0x0A),
                    message,
                    "bs {bs} len {len}"
                );
            }
        }
    }

    #[test]
    fn single_frame_needs_no_flow_control() {
        let mut tx = TxSession::new(&[0x3E, 0x00], 8, Addressing::Normal).unwrap();
        assert!(matches!(tx.poll(), TxAction::SendFrame(_)));
        assert_eq!(tx.poll(), TxAction::Done);
        assert!(tx.is_finished());
    }

    #[test]
    fn block_size_waits_for_flow_control_between_blocks() {
        // BS 2: after 2 CFs the sender waits for the next FlowControl.
        let message: Vec<u8> = (0..30).collect();
        let mut tx = TxSession::new(&message, 8, Addressing::Normal).unwrap();
        assert!(matches!(tx.poll(), TxAction::SendFrame(_))); // FF
        assert_eq!(tx.poll(), TxAction::WaitForFlowControl);
        tx.on_flow_control(FlowControl::new(FlowStatus::ContinueToSend, 2, 0));
        assert!(matches!(tx.poll(), TxAction::SendFrame(_))); // CF1
        assert!(matches!(tx.poll(), TxAction::WaitSeparationTime(_)));
        assert!(matches!(tx.poll(), TxAction::SendFrame(_))); // CF2
        assert_eq!(tx.poll(), TxAction::WaitForFlowControl, "block of 2 done");
    }

    #[test]
    fn overflow_aborts_the_sender() {
        let message: Vec<u8> = (0..30).collect();
        let mut tx = TxSession::new(&message, 8, Addressing::Normal).unwrap();
        tx.poll(); // FF
        tx.poll(); // WaitForFlowControl
        tx.on_flow_control(FlowControl::new(FlowStatus::Overflow, 0, 0));
        assert_eq!(tx.poll(), TxAction::Aborted(Error::BufferOverflow));
        assert!(tx.is_finished());
    }

    #[test]
    fn receiver_overflows_when_message_exceeds_the_limit() {
        let message = vec![0u8; 50];
        let frames = segment_addressed(&message, 8, Addressing::Normal).unwrap();
        let mut rx = RxSession::new(Addressing::Normal, 20, 0, 0, 4);
        let ff = frames[0].encode();
        assert_eq!(
            rx.on_frame(&ff).unwrap(),
            RxAction::SendFlowControl(FlowControl::new(FlowStatus::Overflow, 0, 0))
        );
    }

    #[test]
    fn wait_frames_are_capped_at_wft_max() {
        let mut rx = RxSession::new(Addressing::Normal, 1 << 20, 0, 0, 2);
        // Open a reception so WAITs make sense.
        let frames = segment_addressed(&[0u8; 30], 8, Addressing::Normal).unwrap();
        rx.on_frame(&frames[0].encode()).unwrap();
        assert!(matches!(
            rx.request_wait(),
            Ok(RxAction::SendFlowControl(_))
        ));
        assert!(matches!(
            rx.request_wait(),
            Ok(RxAction::SendFlowControl(_))
        ));
        assert_eq!(rx.request_wait(), Err(Error::WaitFrameOverrun(2)));
    }
}
