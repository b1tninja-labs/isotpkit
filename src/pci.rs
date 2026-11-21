//! The four ISO-TP N_PDU types and their classic-CAN (8-byte) frame codec.
//!
//! Grounded in ISO 15765-2:2016, §9.4 (Transport layer protocol data units) and §9.6 (Protocol
//! control information specification). This module handles classic CAN, where the data-link
//! length `CAN_DL` is at most 8 bytes, and normal addressing (no in-band address byte). CAN FD
//! (`CAN_DL` > 8), the `SingleFrame` escape (`SF_DL` > 7) and the 32-bit `FirstFrame` escape
//! (`FF_DL` > 4095) are planned rows and are rejected here rather than guessed.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::Error;
use crate::flowcontrol::FlowStatus;

/// Classic CAN maximum data length (`CAN_DL`).
pub const CLASSIC_CAN_DL: usize = 8;
/// Default frame padding value (ISO 15765-2:2016, §10.4.2).
pub const DEFAULT_PADDING: u8 = 0xCC;
/// `SingleFrame` payload maximum, normal addressing, `CAN_DL` = 8 (`CAN_DL` − 1).
pub const SF_DL_MAX: usize = 7;
/// `FirstFrame` payload bytes on classic CAN (`CAN_DL` − 2 N_PCI bytes).
pub const FF_DATA_LEN: usize = 6;
/// `ConsecutiveFrame` payload bytes (`CAN_DL` − 1 N_PCI byte).
pub const CF_DATA_MAX: usize = 7;
/// Minimum `FF_DL` (`TX_DL` = 8, normal addressing) (§9.6.3).
pub const FF_DL_MIN: usize = 8;
/// Largest `FF_DL` without the 32-bit escape, 4095 (§9.6.3).
pub const FF_DL_12BIT_MAX: usize = 0xFFF;
/// `SequenceNumber` maximum; it wraps 0x0..=0xF (§9.6.4).
pub const SN_MAX: u8 = 0xF;

/// `N_PCItype`, the high nibble of N_PCI byte #1 (ISO 15765-2:2016, §9.6.1, Table 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PciType {
    /// `SingleFrame` (0x0).
    SingleFrame,
    /// `FirstFrame` (0x1).
    FirstFrame,
    /// `ConsecutiveFrame` (0x2).
    ConsecutiveFrame,
    /// `FlowControl` (0x3).
    FlowControl,
}

impl PciType {
    /// The 4-bit `N_PCItype` value.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            PciType::SingleFrame => 0x0,
            PciType::FirstFrame => 0x1,
            PciType::ConsecutiveFrame => 0x2,
            PciType::FlowControl => 0x3,
        }
    }

    /// Classify N_PCI byte #1 by its high nibble.
    ///
    /// # Errors
    /// [`Error::ReservedPciType`] for the reserved 0x4..=0xF range (§9.6.1, Table 8).
    pub const fn from_byte1(byte1: u8) -> Result<PciType, Error> {
        match byte1 >> 4 {
            0x0 => Ok(PciType::SingleFrame),
            0x1 => Ok(PciType::FirstFrame),
            0x2 => Ok(PciType::ConsecutiveFrame),
            0x3 => Ok(PciType::FlowControl),
            other => Err(Error::ReservedPciType(other)),
        }
    }
}

fn require_frame(frame: &[u8]) -> Result<(), Error> {
    if frame.is_empty() {
        return Err(Error::EmptyFrame);
    }
    if frame.len() > CLASSIC_CAN_DL {
        return Err(Error::FrameTooLong(frame.len()));
    }
    Ok(())
}

fn pad(mut frame: Vec<u8>, padding: u8) -> Vec<u8> {
    frame.resize(CLASSIC_CAN_DL, padding);
    frame
}

/// A `SingleFrame` (SF) N_PDU: an unsegmented message that fits one frame (§9.6.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleFrame {
    data: Vec<u8>,
}

impl SingleFrame {
    /// Build a `SingleFrame` from its payload.
    ///
    /// # Errors
    /// [`Error::SingleFrameLength`] if the payload is empty or longer than 7 bytes.
    pub fn new(data: Vec<u8>) -> Result<SingleFrame, Error> {
        if data.is_empty() || data.len() > SF_DL_MAX {
            return Err(Error::SingleFrameLength(data.len()));
        }
        Ok(SingleFrame { data })
    }

    /// The payload bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// `SF_DL`, the payload length (the low nibble of N_PCI byte #1).
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`new` bounds the payload to 1..=7 bytes, which fits a u8"
    )]
    pub fn sf_dl(&self) -> u8 {
        self.data.len() as u8
    }

    /// Encode to an on-wire frame, padded to 8 bytes with [`DEFAULT_PADDING`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(true, DEFAULT_PADDING)
    }

    /// Encode to an on-wire frame, choosing whether to pad and with which byte.
    ///
    /// With `pad_frame` false the frame uses CAN frame data optimization (§10.4.2): a shortened
    /// `CAN_DL` and no padding.
    #[must_use]
    pub fn encode_with(&self, pad_frame: bool, padding: u8) -> Vec<u8> {
        let mut frame = vec![(PciType::SingleFrame.value() << 4) | self.sf_dl()];
        frame.extend_from_slice(&self.data);
        if pad_frame {
            pad(frame, padding)
        } else {
            frame
        }
    }

    /// Decode an SF frame, validating `SF_DL` against the available data bytes.
    ///
    /// # Errors
    /// [`Error::WrongPciType`] if the frame is not an SF, [`Error::SingleFrameEscape`] for the
    /// CAN FD escape (`SF_DL` 0), and [`Error::SingleFrameLengthExceedsFrame`] if `SF_DL` is
    /// larger than the data the frame carries.
    pub fn decode(frame: &[u8]) -> Result<SingleFrame, Error> {
        require_frame(frame)?;
        if PciType::from_byte1(frame[0])? != PciType::SingleFrame {
            return Err(Error::WrongPciType {
                expected: PciType::SingleFrame,
                byte1: frame[0],
            });
        }
        let sf_dl = usize::from(frame[0] & 0x0F);
        if sf_dl == 0 {
            return Err(Error::SingleFrameEscape);
        }
        let available = frame.len() - 1;
        if sf_dl > available {
            return Err(Error::SingleFrameLengthExceedsFrame { sf_dl, available });
        }
        SingleFrame::new(frame[1..=sf_dl].to_vec())
    }
}

/// A `FirstFrame` (FF) N_PDU: the first frame of a segmented message (§9.6.3).
///
/// `length` is `FF_DL`, the full message length (12-bit, 8..=4095 here). `data` is the first 6
/// payload bytes carried in the FF itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstFrame {
    length: usize,
    data: Vec<u8>,
}

impl FirstFrame {
    /// Build a `FirstFrame` from its `FF_DL` length and its first 6 payload bytes.
    ///
    /// # Errors
    /// [`Error::FirstFrameLength`] if `length` is outside 8..=4095, and
    /// [`Error::FirstFrameDataLength`] if `data` is not exactly 6 bytes.
    pub fn new(length: usize, data: Vec<u8>) -> Result<FirstFrame, Error> {
        if !(FF_DL_MIN..=FF_DL_12BIT_MAX).contains(&length) {
            return Err(Error::FirstFrameLength(length));
        }
        if data.len() != FF_DATA_LEN {
            return Err(Error::FirstFrameDataLength(data.len()));
        }
        Ok(FirstFrame { length, data })
    }

    /// `FF_DL`, the full message length.
    #[must_use]
    pub const fn length(&self) -> usize {
        self.length
    }

    /// The first 6 payload bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Encode to the on-wire FF frame: `0x1<FF_DL hi>` `<FF_DL lo>` then the 6 data bytes.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`length` is 8..=0xFFF, so the shifted and masked parts each fit a u8"
    )]
    pub fn encode(&self) -> Vec<u8> {
        let byte1 = (PciType::FirstFrame.value() << 4) | ((self.length >> 8) & 0x0F) as u8;
        let byte2 = (self.length & 0xFF) as u8;
        let mut frame = vec![byte1, byte2];
        frame.extend_from_slice(&self.data);
        frame
    }

    /// Decode an FF frame, rejecting the 32-bit escape and `CAN_DL` < 8.
    ///
    /// # Errors
    /// [`Error::FirstFrameTooShort`] for `CAN_DL` < 8, [`Error::WrongPciType`] if the frame is
    /// not an FF, [`Error::FirstFrameEscape`] for the 32-bit escape, and
    /// [`Error::FirstFrameLength`] if `FF_DL` is below `FF_DLmin`.
    pub fn decode(frame: &[u8]) -> Result<FirstFrame, Error> {
        require_frame(frame)?;
        if frame.len() < CLASSIC_CAN_DL {
            return Err(Error::FirstFrameTooShort(frame.len()));
        }
        if PciType::from_byte1(frame[0])? != PciType::FirstFrame {
            return Err(Error::WrongPciType {
                expected: PciType::FirstFrame,
                byte1: frame[0],
            });
        }
        let high = usize::from(frame[0] & 0x0F);
        if high == 0 && frame[1] == 0 {
            return Err(Error::FirstFrameEscape);
        }
        let length = (high << 8) | usize::from(frame[1]);
        if length < FF_DL_MIN {
            return Err(Error::FirstFrameLength(length));
        }
        FirstFrame::new(length, frame[2..2 + FF_DATA_LEN].to_vec())
    }
}

/// A `ConsecutiveFrame` (CF) N_PDU: a following frame of a segmented message (§9.6.4).
///
/// `data` holds the payload bytes as read from the frame (up to 7); the last CF of a message
/// may carry fewer meaningful bytes, which the reassembler trims by `FF_DL`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsecutiveFrame {
    sn: u8,
    data: Vec<u8>,
}

impl ConsecutiveFrame {
    /// Build a `ConsecutiveFrame` from its `SequenceNumber` and payload.
    ///
    /// # Errors
    /// [`Error::SequenceNumberRange`] if `sn` is above 15, and [`Error::ConsecutiveFrameLength`]
    /// if the payload is empty or longer than 7 bytes.
    pub fn new(sn: u8, data: Vec<u8>) -> Result<ConsecutiveFrame, Error> {
        if sn > SN_MAX {
            return Err(Error::SequenceNumberRange(sn));
        }
        if data.is_empty() || data.len() > CF_DATA_MAX {
            return Err(Error::ConsecutiveFrameLength(data.len()));
        }
        Ok(ConsecutiveFrame { sn, data })
    }

    /// The `SequenceNumber` (the low nibble of N_PCI byte #1).
    #[must_use]
    pub const fn sn(&self) -> u8 {
        self.sn
    }

    /// The payload bytes as carried in the frame.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Encode to an on-wire frame, padded to 8 bytes with [`DEFAULT_PADDING`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(true, DEFAULT_PADDING)
    }

    /// Encode to an on-wire frame, choosing whether to pad and with which byte.
    #[must_use]
    pub fn encode_with(&self, pad_frame: bool, padding: u8) -> Vec<u8> {
        let mut frame = vec![(PciType::ConsecutiveFrame.value() << 4) | self.sn];
        frame.extend_from_slice(&self.data);
        if pad_frame {
            pad(frame, padding)
        } else {
            frame
        }
    }

    /// Decode a CF frame into its `SequenceNumber` and payload bytes (padding included).
    ///
    /// # Errors
    /// [`Error::WrongPciType`] if the frame is not a CF, and [`Error::ConsecutiveFrameLength`]
    /// if it carries no data byte.
    pub fn decode(frame: &[u8]) -> Result<ConsecutiveFrame, Error> {
        require_frame(frame)?;
        if PciType::from_byte1(frame[0])? != PciType::ConsecutiveFrame {
            return Err(Error::WrongPciType {
                expected: PciType::ConsecutiveFrame,
                byte1: frame[0],
            });
        }
        if frame.len() < 2 {
            return Err(Error::ConsecutiveFrameLength(0));
        }
        ConsecutiveFrame::new(frame[0] & 0x0F, frame[1..].to_vec())
    }
}

/// A `FlowControl` (FC) N_PDU: regulates the `ConsecutiveFrame` rate (§9.6.5).
///
/// `block_size` is `BS` (0 = send all remaining CFs; 1..=255 = CFs per block) and `stmin` is the
/// raw `STmin` byte (parse it with [`crate::flowcontrol::SeparationTime::from_byte`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowControl {
    /// The `FlowStatus` (CTS / WAIT / OVFLW).
    pub flow_status: FlowStatus,
    /// `BlockSize`: 0 sends all remaining CFs, 1..=255 is CFs per block.
    pub block_size: u8,
    /// The raw `STmin` byte.
    pub stmin: u8,
}

impl FlowControl {
    /// Build a `FlowControl` from its `FlowStatus`, `BlockSize` and raw `STmin` byte.
    #[must_use]
    pub const fn new(flow_status: FlowStatus, block_size: u8, stmin: u8) -> FlowControl {
        FlowControl {
            flow_status,
            block_size,
            stmin,
        }
    }

    /// A `ContinueToSend` with `BlockSize` 0 and `STmin` 0.
    #[must_use]
    pub const fn continue_to_send() -> FlowControl {
        FlowControl::new(FlowStatus::ContinueToSend, 0, 0)
    }

    /// Encode to an on-wire frame, padded to 8 bytes with [`DEFAULT_PADDING`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(true, DEFAULT_PADDING)
    }

    /// Encode to the on-wire FC frame: `0x3<FS>` `<BS>` `<STmin>`, optionally padded to 8.
    #[must_use]
    pub fn encode_with(&self, pad_frame: bool, padding: u8) -> Vec<u8> {
        let frame = vec![
            (PciType::FlowControl.value() << 4) | self.flow_status.value(),
            self.block_size,
            self.stmin,
        ];
        if pad_frame {
            pad(frame, padding)
        } else {
            frame
        }
    }

    /// Decode an FC frame, rejecting a reserved `FlowStatus` (`N_INVALID_FS`).
    ///
    /// # Errors
    /// [`Error::FlowControlTooShort`] if the frame has fewer than 3 N_PCI bytes,
    /// [`Error::WrongPciType`] if it is not an FC, and [`Error::ReservedFlowStatus`] for a
    /// reserved `FlowStatus`.
    pub fn decode(frame: &[u8]) -> Result<FlowControl, Error> {
        require_frame(frame)?;
        if frame.len() < 3 {
            return Err(Error::FlowControlTooShort(frame.len()));
        }
        if PciType::from_byte1(frame[0])? != PciType::FlowControl {
            return Err(Error::WrongPciType {
                expected: PciType::FlowControl,
                byte1: frame[0],
            });
        }
        let flow_status = FlowStatus::from_nibble(frame[0] & 0x0F)?;
        Ok(FlowControl::new(flow_status, frame[1], frame[2]))
    }
}

/// Any one of the four N_PDU types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NPdu {
    /// A `SingleFrame`.
    Single(SingleFrame),
    /// A `FirstFrame`.
    First(FirstFrame),
    /// A `ConsecutiveFrame`.
    Consecutive(ConsecutiveFrame),
    /// A `FlowControl`.
    Flow(FlowControl),
}

/// Decode any classic-CAN ISO-TP frame into its N_PDU, dispatched on `N_PCItype`.
///
/// # Errors
/// Any [`Error`] the per-type decoder returns, including [`Error::ReservedPciType`] for the
/// reserved `N_PCItype` range.
pub fn decode_frame(frame: &[u8]) -> Result<NPdu, Error> {
    require_frame(frame)?;
    match PciType::from_byte1(frame[0])? {
        PciType::SingleFrame => SingleFrame::decode(frame).map(NPdu::Single),
        PciType::FirstFrame => FirstFrame::decode(frame).map(NPdu::First),
        PciType::ConsecutiveFrame => ConsecutiveFrame::decode(frame).map(NPdu::Consecutive),
        PciType::FlowControl => FlowControl::decode(frame).map(NPdu::Flow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_pci_type_is_rejected() {
        assert_eq!(PciType::from_byte1(0x40), Err(Error::ReservedPciType(4)));
        assert_eq!(PciType::from_byte1(0xF0), Err(Error::ReservedPciType(0xF)));
        assert_eq!(PciType::from_byte1(0x30), Ok(PciType::FlowControl));
    }

    #[test]
    fn single_frame_encodes_pci_and_pads() {
        let sf = SingleFrame::new(vec![0x11, 0x22]).unwrap();
        assert_eq!(sf.sf_dl(), 2);
        assert_eq!(
            sf.encode(),
            vec![0x02, 0x11, 0x22, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC]
        );
        assert_eq!(
            sf.encode_with(false, DEFAULT_PADDING),
            vec![0x02, 0x11, 0x22]
        );
    }

    #[test]
    fn single_frame_decode_roundtrip() {
        let sf = SingleFrame::new(vec![0x3E, 0x00]).unwrap();
        assert_eq!(SingleFrame::decode(&sf.encode()).unwrap(), sf);
        assert_eq!(
            SingleFrame::decode(&[0x00, 1]),
            Err(Error::SingleFrameEscape)
        );
        assert_eq!(
            SingleFrame::decode(&[0x05, 1, 2]),
            Err(Error::SingleFrameLengthExceedsFrame {
                sf_dl: 5,
                available: 2
            })
        );
    }

    #[test]
    fn first_frame_encodes_12bit_length() {
        let ff = FirstFrame::new(0x123, vec![1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(ff.encode(), vec![0x11, 0x23, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn first_frame_decode_roundtrip_and_rejects_short() {
        let ff = FirstFrame::new(4095, vec![9, 8, 7, 6, 5, 4]).unwrap();
        assert_eq!(FirstFrame::decode(&ff.encode()).unwrap(), ff);
        assert_eq!(
            FirstFrame::decode(&[0x10, 0x05, 1, 2, 3, 4, 5, 6]),
            Err(Error::FirstFrameLength(5))
        );
        assert_eq!(
            FirstFrame::decode(&[0x10, 0x08, 1, 2, 3, 4]),
            Err(Error::FirstFrameTooShort(6))
        );
        assert_eq!(
            FirstFrame::decode(&[0x10, 0x00, 0, 0, 0, 0, 0, 0]),
            Err(Error::FirstFrameEscape)
        );
    }

    #[test]
    fn consecutive_frame_encode_and_decode() {
        let cf = ConsecutiveFrame::new(5, vec![1, 2, 3]).unwrap();
        assert_eq!(cf.encode_with(false, DEFAULT_PADDING), vec![0x25, 1, 2, 3]);
        let decoded = ConsecutiveFrame::decode(&cf.encode()).unwrap();
        assert_eq!(decoded.sn(), 5);
        assert_eq!(&decoded.data()[..3], &[1, 2, 3]);
    }

    #[test]
    fn flow_control_encode_and_decode() {
        let fc = FlowControl::new(FlowStatus::Wait, 8, 0x14);
        assert_eq!(
            fc.encode(),
            vec![0x31, 8, 0x14, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC]
        );
        assert_eq!(FlowControl::decode(&fc.encode()).unwrap(), fc);
        assert_eq!(
            FlowControl::decode(&[0x33, 0, 0]),
            Err(Error::ReservedFlowStatus(3))
        );
        assert_eq!(
            FlowControl::decode(&[0x30, 0]),
            Err(Error::FlowControlTooShort(2))
        );
    }

    #[test]
    fn decode_frame_dispatches_on_type() {
        let sf = SingleFrame::new(vec![0x7F]).unwrap();
        assert_eq!(decode_frame(&sf.encode()).unwrap(), NPdu::Single(sf));
        assert_eq!(decode_frame(&[]), Err(Error::EmptyFrame));
        assert_eq!(
            decode_frame(&[0, 1, 2, 3, 4, 5, 6, 7, 8]),
            Err(Error::FrameTooLong(9))
        );
    }
}
