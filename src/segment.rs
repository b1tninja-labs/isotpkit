//! Segmentation and reassembly of an application message across ISO-TP N_PDUs.
//!
//! Grounded in ISO 15765-2:2016, §9.3 (Multiple-frame transmission) and §9.6.4 (`ConsecutiveFrame`
//! `SequenceNumber`). Segmentation is pure: it emits the N_PDU sequence a sender would send,
//! without the `FlowControl` pacing that needs a live bus. Reassembly validates `SequenceNumbers`.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::Error;
use crate::pci::{
    Addressing, CLASSIC_CAN_DL, ConsecutiveFrame, FF_DL_12BIT_MAX, FF_DL_32BIT_MAX, FirstFrame,
    NPdu, SF_DL_MAX, SN_MAX, SingleFrame, VALID_CAN_DL, encode_addressed,
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
    /// A `FirstFrame` fills its frame exactly, so `pad` and `padding` affect only a `SingleFrame`
    /// or the last `ConsecutiveFrame` (CAN frame data optimization, §10.4.2); a CAN FD frame is
    /// padded up to its `CAN_DL` regardless (mandatory, §10.4.2.3).
    #[must_use]
    pub fn encode_with(&self, pad: bool, padding: u8) -> Vec<u8> {
        match self {
            SegmentPdu::Single(sf) => sf.encode_with(pad, padding),
            SegmentPdu::First(ff) => ff.encode(),
            SegmentPdu::Consecutive(cf) => cf.encode_with(pad, padding),
        }
    }

    /// Encode the segment for a given addressing format (§10.3), prepending the in-band address
    /// byte for extended/mixed addressing. Pair with [`segment_addressed`] so the sizes fit.
    #[must_use]
    pub fn encode_addressed(&self, addressing: Addressing, padding: u8) -> Vec<u8> {
        let npdu = match self {
            SegmentPdu::Single(sf) => NPdu::Single(sf.clone()),
            SegmentPdu::First(ff) => NPdu::First(ff.clone()),
            SegmentPdu::Consecutive(cf) => NPdu::Consecutive(cf.clone()),
        };
        encode_addressed(&npdu, addressing, padding)
    }
}

/// Split an application message into its ISO-TP N_PDU sequence for classic CAN (`TX_DL` = 8).
///
/// A short message becomes one `SingleFrame`; a longer one becomes a `FirstFrame` (12-bit or
/// 32-bit-escape length) followed by `ConsecutiveFrames` whose `SequenceNumber` starts at 1 and
/// wraps 0xF → 0x0. Equivalent to [`segment_with_tx_dl`] with `tx_dl` = 8.
///
/// # Errors
/// [`Error::EmptyMessage`] for an empty message, and [`Error::MessageTooLong`] for one over
/// 4294967295 bytes.
pub fn segment(message: &[u8]) -> Result<Vec<SegmentPdu>, Error> {
    segment_with_tx_dl(message, CLASSIC_CAN_DL)
}

/// Split an application message into its ISO-TP N_PDU sequence for a given `TX_DL`.
///
/// `tx_dl` is the configured CAN data length (§9.5): 8 for classic CAN, or a CAN FD value
/// (12/16/20/24/32/48/64). The `SingleFrame`, `FirstFrame` and `ConsecutiveFrame` payload sizes,
/// and the choice of the 12-bit or 32-bit `FF_DL` form, follow from it.
///
/// # Errors
/// [`Error::EmptyMessage`] for an empty message, [`Error::InvalidCanDl`] for a `tx_dl` that is not
/// a valid `CAN_DL`, and [`Error::MessageTooLong`] for a message over 4294967295 bytes.
pub fn segment_with_tx_dl(message: &[u8], tx_dl: usize) -> Result<Vec<SegmentPdu>, Error> {
    if message.is_empty() {
        return Err(Error::EmptyMessage);
    }
    if !VALID_CAN_DL.contains(&tx_dl) {
        return Err(Error::InvalidCanDl(tx_dl));
    }
    if message.len() > FF_DL_32BIT_MAX {
        return Err(Error::MessageTooLong(message.len()));
    }
    // SingleFrame payload capacity for this TX_DL, normal addressing: 7 on classic, TX_DL − 2 FD.
    let sf_max = if tx_dl == CLASSIC_CAN_DL {
        SF_DL_MAX
    } else {
        tx_dl - 2
    };
    if message.len() <= sf_max {
        return Ok(vec![SegmentPdu::Single(SingleFrame::new(
            message.to_vec(),
        )?)]);
    }
    // FirstFrame payload: TX_DL minus the N_PCI bytes (2 for the 12-bit form, 6 for the escape).
    let ff_pci = if message.len() <= FF_DL_12BIT_MAX {
        2
    } else {
        6
    };
    let ff_data_len = tx_dl - ff_pci;
    let mut pdus = vec![SegmentPdu::First(FirstFrame::new(
        message.len(),
        message[..ff_data_len].to_vec(),
    )?)];
    let cf_data = tx_dl - 1;
    let mut sn: u8 = 1;
    let mut offset = ff_data_len;
    while offset < message.len() {
        let end = (offset + cf_data).min(message.len());
        pdus.push(SegmentPdu::Consecutive(ConsecutiveFrame::new(
            sn,
            message[offset..end].to_vec(),
        )?));
        sn = (sn + 1) & SN_MAX;
        offset = end;
    }
    Ok(pdus)
}

/// Split a message into N_PDUs sized for a given `TX_DL` and addressing format (§10.3).
///
/// Like [`segment_with_tx_dl`], but an in-band address byte (extended/mixed) costs one payload
/// byte per frame. Encode each result with [`SegmentPdu::encode_addressed`] using the same
/// `addressing`.
///
/// # Errors
/// [`Error::EmptyMessage`], [`Error::InvalidCanDl`] for a bad `tx_dl`, and [`Error::MessageTooLong`].
pub fn segment_addressed(
    message: &[u8],
    tx_dl: usize,
    addressing: Addressing,
) -> Result<Vec<SegmentPdu>, Error> {
    if message.is_empty() {
        return Err(Error::EmptyMessage);
    }
    if !VALID_CAN_DL.contains(&tx_dl) {
        return Err(Error::InvalidCanDl(tx_dl));
    }
    if message.len() > FF_DL_32BIT_MAX {
        return Err(Error::MessageTooLong(message.len()));
    }
    let header = addressing.header_len();
    let sf_base = if tx_dl == CLASSIC_CAN_DL {
        SF_DL_MAX
    } else {
        tx_dl - 2
    };
    let sf_max = sf_base - header;
    if message.len() <= sf_max {
        return Ok(vec![SegmentPdu::Single(SingleFrame::new(
            message.to_vec(),
        )?)]);
    }
    let ff_pci = if message.len() <= FF_DL_12BIT_MAX {
        2
    } else {
        6
    };
    let ff_data_len = tx_dl - header - ff_pci;
    let mut pdus = vec![SegmentPdu::First(FirstFrame::new(
        message.len(),
        message[..ff_data_len].to_vec(),
    )?)];
    let cf_data = tx_dl - header - 1;
    let mut sn: u8 = 1;
    let mut offset = ff_data_len;
    while offset < message.len() {
        let end = (offset + cf_data).min(message.len());
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
    use crate::pci::{CF_DATA_MAX, FF_DATA_LEN};

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

        // The FirstFrame of a classic multi-frame message always fills 8 bytes, padding or not.
        let multi = segment(&[9; 20]).unwrap();
        assert_eq!(multi[0].encode_with(false, 0x00), multi[0].encode());
        assert_eq!(multi[0].encode().len(), 8);
    }

    #[test]
    fn can_fd_segmentation_round_trips() {
        // TX_DL 64: the FirstFrame and ConsecutiveFrames carry CAN FD payloads.
        for len in [63usize, 100, 500, 4095, 4096, 10_000] {
            let message: Vec<u8> = (0..len).map(|i| (i * 11 + 1) as u8).collect();
            let pdus = segment_with_tx_dl(&message, 64).unwrap();
            assert!(matches!(pdus[0], SegmentPdu::First(_)), "len {len}");
            // Every encoded frame is a valid CAN FD CAN_DL (≤ 64).
            for pdu in &pdus {
                let frame = pdu.encode();
                assert!(
                    frame.len() <= 64
                        && (frame.len() <= 8 || crate::pci::VALID_CAN_DL.contains(&frame.len()))
                );
            }
            assert_eq!(reassemble(&pdus).unwrap(), message, "len {len}");
        }
        assert_eq!(segment_with_tx_dl(b"x", 10), Err(Error::InvalidCanDl(10)));
    }

    #[test]
    fn classic_can_uses_the_32bit_escape_past_4095() {
        // On classic CAN (TX_DL 8) a message over 4095 bytes uses the 32-bit FF_DL escape.
        let message = vec![0x5Au8; 5000];
        let pdus = segment(&message).unwrap();
        match &pdus[0] {
            SegmentPdu::First(ff) => {
                assert_eq!(ff.length(), 5000);
                assert_eq!(&ff.encode()[..2], &[0x10, 0x00], "32-bit escape marker");
            }
            other => panic!("expected a FirstFrame, got {other:?}"),
        }
        assert_eq!(reassemble(&pdus).unwrap(), message);
    }

    #[test]
    fn extended_addressing_segmentation_round_trips() {
        let addr = Addressing::Extended(0xF1);
        for len in [6usize, 7, 50, 300] {
            let message: Vec<u8> = (0..len).map(|i| (i * 5 + 1) as u8).collect();
            let pdus = segment_addressed(&message, CLASSIC_CAN_DL, addr).unwrap();
            // Encode each frame with its address byte, then decode it back and reassemble.
            let decoded: Vec<SegmentPdu> = pdus
                .iter()
                .map(|pdu| {
                    let frame = pdu.encode_addressed(addr, 0xCC);
                    assert_eq!(frame[0], 0xF1, "address byte leads (len {len})");
                    match crate::pci::decode_addressed(&frame, addr).unwrap() {
                        NPdu::Single(sf) => SegmentPdu::Single(sf),
                        NPdu::First(ff) => SegmentPdu::First(ff),
                        NPdu::Consecutive(cf) => SegmentPdu::Consecutive(cf),
                        NPdu::Flow(_) => unreachable!(),
                    }
                })
                .collect();
            assert_eq!(reassemble(&decoded).unwrap(), message, "len {len}");
        }
    }
}
