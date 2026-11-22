//! Segmentation and reassembly of an application message across ISO-TP N_PDUs.
//!
//! Grounded in ISO 15765-2:2016, §9.3 (Multiple-frame transmission) and §9.6.4 (`ConsecutiveFrame`
//! `SequenceNumber`). Segmentation is pure: it emits the N_PDU sequence a sender would send,
//! without the `FlowControl` pacing that needs a live bus. Reassembly validates `SequenceNumbers`.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::Error;
use crate::pci::{
    CF_DATA_MAX, ConsecutiveFrame, FF_DATA_LEN, FF_DL_12BIT_MAX, FirstFrame, SF_DL_MAX, SN_MAX,
    SingleFrame,
};

/// One of the three N_PDU types that carry message data (SF, FF, CF). `FlowControl` is not a
/// data-carrying segment, so it is not part of a segmentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentPdu {
    /// A `SingleFrame`.
    Single(SingleFrame),
    /// A `FirstFrame`.
    First(FirstFrame),
    /// A `ConsecutiveFrame`.
    Consecutive(ConsecutiveFrame),
}

impl SegmentPdu {
    /// Encode the segment to its on-wire frame, padded to 8 bytes with the default padding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            SegmentPdu::Single(sf) => sf.encode(),
            SegmentPdu::First(ff) => ff.encode(),
            SegmentPdu::Consecutive(cf) => cf.encode(),
        }
    }

    /// Encode the segment, choosing whether to pad and with which byte.
    ///
    /// A `FirstFrame` always fills all 8 bytes, so `pad` and `padding` affect only a `SingleFrame`
    /// or the last `ConsecutiveFrame` (CAN frame data optimization, §10.4.2).
    #[must_use]
    pub fn encode_with(&self, pad: bool, padding: u8) -> Vec<u8> {
        match self {
            SegmentPdu::Single(sf) => sf.encode_with(pad, padding),
            SegmentPdu::First(ff) => ff.encode(),
            SegmentPdu::Consecutive(cf) => cf.encode_with(pad, padding),
        }
    }
}

/// Split an application message into its ISO-TP N_PDU sequence.
///
/// A message of 1..=7 bytes becomes one `SingleFrame`. A longer message becomes a `FirstFrame`
/// (carrying the 12-bit length and the first 6 bytes) followed by `ConsecutiveFrames` whose
/// `SequenceNumber` starts at 1 and wraps 0xF → 0x0.
///
/// # Errors
/// [`Error::EmptyMessage`] for an empty message, and [`Error::MessageTooLong`] for one over 4095
/// bytes (the 32-bit `FF_DL` escape is a planned row).
pub fn segment(message: &[u8]) -> Result<Vec<SegmentPdu>, Error> {
    if message.is_empty() {
        return Err(Error::EmptyMessage);
    }
    if message.len() <= SF_DL_MAX {
        return Ok(vec![SegmentPdu::Single(SingleFrame::new(
            message.to_vec(),
        )?)]);
    }
    if message.len() > FF_DL_12BIT_MAX {
        return Err(Error::MessageTooLong(message.len()));
    }
    let mut pdus = vec![SegmentPdu::First(FirstFrame::new(
        message.len(),
        message[..FF_DATA_LEN].to_vec(),
    )?)];
    let mut sn: u8 = 1;
    let mut offset = FF_DATA_LEN;
    while offset < message.len() {
        let end = (offset + CF_DATA_MAX).min(message.len());
        pdus.push(SegmentPdu::Consecutive(ConsecutiveFrame::new(
            sn,
            message[offset..end].to_vec(),
        )?));
        sn = (sn + 1) & SN_MAX;
        offset = end;
    }
    Ok(pdus)
}

/// Consume an ordered N_PDU sequence back into the application message.
///
/// [`Reassembler::feed`] returns the completed message once the last frame arrives, otherwise
/// `None`. A `SingleFrame` completes immediately; a `FirstFrame` opens a session and
/// `ConsecutiveFrames` fill it in order. A `ConsecutiveFrame` with the wrong `SequenceNumber` is
/// the `N_WRONG_SN` condition of §9.6.4.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    expected: Option<usize>,
    buffer: Vec<u8>,
    next_sn: u8,
}

impl Reassembler {
    /// A fresh reassembler with no message in progress.
    #[must_use]
    pub fn new() -> Reassembler {
        Reassembler {
            expected: None,
            buffer: Vec::new(),
            next_sn: 1,
        }
    }

    /// Whether a `FirstFrame` has opened a message that is not yet complete.
    #[must_use]
    pub const fn in_progress(&self) -> bool {
        self.expected.is_some()
    }

    /// Discard any in-progress reassembly state.
    pub fn reset(&mut self) {
        self.expected = None;
        self.buffer.clear();
        self.next_sn = 1;
    }

    /// Feed the next N_PDU; return `Some(message)` when it completes, else `None`.
    ///
    /// # Errors
    /// [`Error::SingleFrameDuringReassembly`] / [`Error::FirstFrameDuringReassembly`] when a new
    /// message starts mid-reassembly, [`Error::ConsecutiveFrameWithoutFirstFrame`] for a CF with
    /// no open message, and [`Error::WrongSequenceNumber`] for a CF out of order (`N_WRONG_SN`).
    pub fn feed(&mut self, pdu: &SegmentPdu) -> Result<Option<Vec<u8>>, Error> {
        match pdu {
            SegmentPdu::Single(sf) => {
                if self.expected.is_some() {
                    return Err(Error::SingleFrameDuringReassembly);
                }
                Ok(Some(sf.data().to_vec()))
            }
            SegmentPdu::First(ff) => {
                if self.expected.is_some() {
                    return Err(Error::FirstFrameDuringReassembly);
                }
                self.expected = Some(ff.length());
                self.buffer = ff.data().to_vec();
                self.next_sn = 1;
                Ok(None)
            }
            SegmentPdu::Consecutive(cf) => {
                let expected = self
                    .expected
                    .ok_or(Error::ConsecutiveFrameWithoutFirstFrame)?;
                if cf.sn() != self.next_sn {
                    let err = Error::WrongSequenceNumber {
                        expected: self.next_sn,
                        got: cf.sn(),
                    };
                    self.reset();
                    return Err(err);
                }
                let remaining = expected - self.buffer.len();
                let take = remaining.min(cf.data().len());
                self.buffer.extend_from_slice(&cf.data()[..take]);
                self.next_sn = (self.next_sn + 1) & SN_MAX;
                if self.buffer.len() >= expected {
                    let message = self.buffer[..expected].to_vec();
                    self.reset();
                    Ok(Some(message))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

/// Reassemble a complete N_PDU sequence into its message.
///
/// # Errors
/// Any error [`Reassembler::feed`] returns, and [`Error::IncompleteSequence`] if the sequence
/// ends without completing a message.
pub fn reassemble(pdus: &[SegmentPdu]) -> Result<Vec<u8>, Error> {
    let mut reassembler = Reassembler::new();
    let mut message = None;
    for pdu in pdus {
        message = reassembler.feed(pdu)?;
    }
    message.ok_or(Error::IncompleteSequence)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        reason = "test fixtures build byte patterns from loop indices"
    )]
    use super::*;

    #[test]
    fn short_message_is_a_single_frame() {
        let pdus = segment(&[1, 2, 3]).unwrap();
        assert_eq!(pdus.len(), 1);
        assert!(matches!(pdus[0], SegmentPdu::Single(_)));
        assert_eq!(segment(&[]), Err(Error::EmptyMessage));
    }

    #[test]
    fn long_message_splits_into_ff_and_cfs() {
        let message: Vec<u8> = (0..20).collect();
        let pdus = segment(&message).unwrap();
        assert!(matches!(pdus[0], SegmentPdu::First(_)));
        match &pdus[1] {
            SegmentPdu::Consecutive(cf) => assert_eq!(cf.sn(), 1),
            other => panic!("expected a ConsecutiveFrame, got {other:?}"),
        }
    }

    #[test]
    fn sequence_number_wraps_past_15() {
        // 6 bytes in the FF, then 16 CFs of 7 bytes: SN runs 1..=15, 0, and the 16th CF is SN 0.
        let message = vec![0x5A; FF_DATA_LEN + CF_DATA_MAX * 16];
        let pdus = segment(&message).unwrap();
        let sns: Vec<u8> = pdus
            .iter()
            .filter_map(|p| match p {
                SegmentPdu::Consecutive(cf) => Some(cf.sn()),
                _ => None,
            })
            .collect();
        assert_eq!(sns.len(), 16);
        assert_eq!(sns[14], 15);
        assert_eq!(sns[15], 0);
    }

    #[test]
    fn reassemble_round_trips_segmentation() {
        for len in 1..=300usize {
            let message: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
            let pdus = segment(&message).unwrap();
            assert_eq!(reassemble(&pdus).unwrap(), message, "len {len}");
        }
        let big = vec![0x42; FF_DL_12BIT_MAX];
        assert_eq!(reassemble(&segment(&big).unwrap()).unwrap(), big);
    }

    #[test]
    fn single_frame_reassembles_immediately() {
        let sf = SingleFrame::new(vec![0x3E, 0x00]).unwrap();
        let mut r = Reassembler::new();
        assert_eq!(
            r.feed(&SegmentPdu::Single(sf)).unwrap(),
            Some(vec![0x3E, 0x00])
        );
        assert!(!r.in_progress());
    }

    #[test]
    fn wrong_sequence_number_is_rejected() {
        let pdus = segment(&[7; 30]).unwrap();
        let mut r = Reassembler::new();
        r.feed(&pdus[0]).unwrap(); // FF
        assert_eq!(
            r.feed(&pdus[2]), // skips SN 1, feeds SN 2
            Err(Error::WrongSequenceNumber {
                expected: 1,
                got: 2
            })
        );
        assert!(!r.in_progress());
    }

    #[test]
    fn consecutive_frame_without_first_frame_is_rejected() {
        let cf = ConsecutiveFrame::new(1, vec![1, 2, 3]).unwrap();
        let mut r = Reassembler::new();
        assert_eq!(
            r.feed(&SegmentPdu::Consecutive(cf)),
            Err(Error::ConsecutiveFrameWithoutFirstFrame)
        );
    }

    #[test]
    fn segment_pdu_encode_with_controls_padding() {
        let single = &segment(&[1, 2, 3]).unwrap()[0];
        assert_eq!(single.encode_with(false, 0x00), vec![0x03, 1, 2, 3]);
        assert_eq!(
            single.encode_with(true, 0xAA),
            vec![0x03, 1, 2, 3, 0xAA, 0xAA, 0xAA, 0xAA]
        );

        // The FirstFrame of a multi-frame message always fills 8 bytes, padding or not.
        let multi = segment(&[9; 20]).unwrap();
        assert_eq!(multi[0].encode_with(false, 0x00), multi[0].encode());
        assert_eq!(multi[0].encode().len(), 8);
    }
}
