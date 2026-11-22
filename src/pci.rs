//! The four ISO-TP N_PDU types and their classic-CAN and CAN FD frame codec.
//!
//! Grounded in ISO 15765-2:2016, §9.4 (Transport layer protocol data units), §9.5 (TX_DL/RX_DL)
//! and §9.6 (Protocol control information specification). Normal addressing only (no in-band
//! address byte). Classic CAN (`CAN_DL` ≤ 8) and CAN FD (`CAN_DL` one of 12/16/20/24/32/48/64)
//! are both handled, including the `SingleFrame` escape (`SF_DL` > 7, in N_PCI byte #2) and the
//! 32-bit `FirstFrame` escape (`FF_DL` > 4095, in N_PCI bytes #3..#6). Extended/mixed addressing
//! is a planned row.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::Error;
use crate::flowcontrol::FlowStatus;

/// Classic CAN maximum data length (`CAN_DL`).
pub const CLASSIC_CAN_DL: usize = 8;
/// CAN FD maximum data length (`CAN_DL`).
pub const CAN_FD_MAX_DL: usize = 64;
/// The valid `CAN_DL` / `RX_DL` values (§9.5.4, Table 7): classic 8, CAN FD 12..=64.
pub const VALID_CAN_DL: [usize; 8] = [8, 12, 16, 20, 24, 32, 48, 64];
/// Default frame padding value (ISO 15765-2:2016, §10.4.2).
pub const DEFAULT_PADDING: u8 = 0xCC;
/// Largest classic `SingleFrame` payload, normal addressing (`CAN_DL` 8 − 1 N_PCI byte).
pub const SF_DL_MAX: usize = 7;
/// Largest CAN FD `SingleFrame` payload, normal addressing (`CAN_DL` 64 − 2 N_PCI bytes).
pub const SF_DL_FD_MAX: usize = 62;
/// `FirstFrame` payload bytes on classic CAN (`CAN_DL` 8 − 2 N_PCI bytes).
pub const FF_DATA_LEN: usize = 6;
/// Largest classic `ConsecutiveFrame` payload (`CAN_DL` 8 − 1 N_PCI byte).
pub const CF_DATA_MAX: usize = 7;
/// Largest CAN FD `ConsecutiveFrame` payload (`CAN_DL` 64 − 1 N_PCI byte).
pub const CF_DATA_FD_MAX: usize = 63;
/// Minimum `FF_DL` (`TX_DL` = 8, normal addressing) (§9.6.3, Table 14).
pub const FF_DL_MIN: usize = 8;
/// Minimum `FF_DL` (`TX_DL` = 8, extended/mixed addressing) (§9.6.3, Table 14).
pub const FF_DL_MIN_EXTENDED: usize = 7;
/// Largest `FF_DL` without the 32-bit escape, 4095 (§9.6.3).
pub const FF_DL_12BIT_MAX: usize = 0xFFF;
/// Largest `FF_DL` with the 32-bit escape, 4294967295 (§9.6.3).
pub const FF_DL_32BIT_MAX: usize = u32::MAX as usize;
/// `SequenceNumber` maximum; it wraps 0x0..=0xF (§9.6.4).
pub const SN_MAX: u8 = 0xF;

/// The smallest valid `CAN_DL` that holds `content` N_PCI+data bytes (content must be ≤ 64).
fn can_dl_for(content: usize) -> usize {
    VALID_CAN_DL
        .iter()
        .copied()
        .find(|&dl| dl >= content)
        .unwrap_or(CAN_FD_MAX_DL)
}

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
    if frame.len() > CAN_FD_MAX_DL {
        return Err(Error::FrameTooLong(frame.len()));
    }
    if frame.len() > CLASSIC_CAN_DL && !VALID_CAN_DL.contains(&frame.len()) {
        return Err(Error::InvalidCanDl(frame.len()));
    }
    Ok(())
}

fn pad_to(mut frame: Vec<u8>, can_dl: usize, padding: u8) -> Vec<u8> {
    frame.resize(can_dl, padding);
    frame
}

/// A `SingleFrame` (SF) N_PDU: an unsegmented message that fits one frame (§9.6.2).
///
/// A payload of 1..=7 bytes encodes as a classic SF (`SF_DL` in N_PCI byte #1); 8..=62 bytes
/// encode as the CAN FD escape (byte #1 low nibble 0, `SF_DL` in byte #2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleFrame {
    data: Vec<u8>,
}

impl SingleFrame {
    /// Build a `SingleFrame` from its payload.
    ///
    /// # Errors
    /// [`Error::SingleFrameLength`] if the payload is empty or longer than 62 bytes.
    pub fn new(data: Vec<u8>) -> Result<SingleFrame, Error> {
        if data.is_empty() || data.len() > SF_DL_FD_MAX {
            return Err(Error::SingleFrameLength(data.len()));
        }
        Ok(SingleFrame { data })
    }

    /// The payload bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// `SF_DL`, the payload length.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`new` bounds the payload to 1..=62 bytes, which fits a u8"
    )]
    pub fn sf_dl(&self) -> u8 {
        self.data.len() as u8
    }

    /// Encode to an on-wire frame with the default padding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(true, DEFAULT_PADDING)
    }

    /// Encode to an on-wire frame.
    ///
    /// A classic SF (`SF_DL` ≤ 7) honours `pad_frame` (false uses CAN frame data optimization,
    /// §10.4.2.2). A CAN FD SF (`SF_DL` > 7) is always padded up to the smallest valid `CAN_DL`
    /// — padding is mandatory for CAN FD (§10.4.2.3).
    #[must_use]
    pub fn encode_with(&self, pad_frame: bool, padding: u8) -> Vec<u8> {
        if self.data.len() <= SF_DL_MAX {
            let mut frame = vec![(PciType::SingleFrame.value() << 4) | self.sf_dl()];
            frame.extend_from_slice(&self.data);
            if pad_frame {
                pad_to(frame, CLASSIC_CAN_DL, padding)
            } else {
                frame
            }
        } else {
            let mut frame = vec![PciType::SingleFrame.value() << 4, self.sf_dl()];
            frame.extend_from_slice(&self.data);
            let can_dl = can_dl_for(frame.len());
            pad_to(frame, can_dl, padding)
        }
    }

    /// Decode an SF frame (classic or CAN FD escape), validating `SF_DL`.
    ///
    /// # Errors
    /// [`Error::WrongPciType`] if not an SF, [`Error::ReservedSingleFrame`] for `SF_DL` 0 on
    /// classic CAN or a non-zero low nibble on a CAN FD escape, and
    /// [`Error::SingleFrameLengthExceedsFrame`] if `SF_DL` does not fit the frame.
    pub fn decode(frame: &[u8]) -> Result<SingleFrame, Error> {
        require_frame(frame)?;
        if PciType::from_byte1(frame[0])? != PciType::SingleFrame {
            return Err(Error::WrongPciType {
                expected: PciType::SingleFrame,
                byte1: frame[0],
            });
        }
        if frame.len() <= CLASSIC_CAN_DL {
            let sf_dl = usize::from(frame[0] & 0x0F);
            if sf_dl == 0 {
                return Err(Error::ReservedSingleFrame);
            }
            let available = frame.len() - 1;
            if sf_dl > available {
                return Err(Error::SingleFrameLengthExceedsFrame { sf_dl, available });
            }
            SingleFrame::new(frame[1..=sf_dl].to_vec())
        } else {
            if frame[0] & 0x0F != 0 {
                return Err(Error::ReservedSingleFrame);
            }
            let sf_dl = usize::from(frame[1]);
            let available = frame.len() - 2;
            if sf_dl <= SF_DL_MAX || sf_dl > available {
                return Err(Error::SingleFrameLengthExceedsFrame { sf_dl, available });
            }
            SingleFrame::new(frame[2..2 + sf_dl].to_vec())
        }
    }
}

/// A `FirstFrame` (FF) N_PDU: the first frame of a segmented message (§9.6.3).
///
/// `length` is `FF_DL`, the full message length. 8..=4095 encodes as the 12-bit form; 4096..=2³²−1
/// encodes as the 32-bit escape (byte #1 low nibble 0, byte #2 0, `FF_DL` in bytes #3..#6). `data`
/// is the first payload chunk; its length plus the N_PCI bytes must make a valid `CAN_DL`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstFrame {
    length: usize,
    data: Vec<u8>,
}

impl FirstFrame {
    /// Build a `FirstFrame` from its `FF_DL` length and its first payload chunk.
    ///
    /// The chunk length depends on the `TX_DL` and the addressing format (one byte smaller with an
    /// in-band address byte), so it is only bounded to fit a frame here; the segmenter sizes it.
    ///
    /// # Errors
    /// [`Error::FirstFrameLength`] if `length` is outside 8..=4294967295, and
    /// [`Error::FirstFrameDataLength`] if `data` is empty or cannot fit a frame with its N_PCI.
    pub fn new(length: usize, data: Vec<u8>) -> Result<FirstFrame, Error> {
        if !(FF_DL_MIN_EXTENDED..=FF_DL_32BIT_MAX).contains(&length) {
            return Err(Error::FirstFrameLength(length));
        }
        let pci_len = if length <= FF_DL_12BIT_MAX { 2 } else { 6 };
        if data.is_empty() || pci_len + data.len() > CAN_FD_MAX_DL {
            return Err(Error::FirstFrameDataLength(data.len()));
        }
        Ok(FirstFrame { length, data })
    }

    /// `FF_DL`, the full message length.
    #[must_use]
    pub const fn length(&self) -> usize {
        self.length
    }

    /// The first payload chunk.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Encode to the on-wire FF frame (12-bit or 32-bit-escape form).
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`length` is 8..=u32::MAX; the shifted/masked parts and the u32 cast each fit"
    )]
    pub fn encode(&self) -> Vec<u8> {
        let mut frame = if self.length <= FF_DL_12BIT_MAX {
            vec![
                (PciType::FirstFrame.value() << 4) | ((self.length >> 8) & 0x0F) as u8,
                (self.length & 0xFF) as u8,
            ]
        } else {
            let mut escape = vec![PciType::FirstFrame.value() << 4, 0u8];
            escape.extend_from_slice(&(self.length as u32).to_be_bytes());
            escape
        };
        frame.extend_from_slice(&self.data);
        frame
    }

    /// Decode an FF frame (12-bit or 32-bit escape), rejecting `CAN_DL` < 8.
    ///
    /// # Errors
    /// [`Error::FirstFrameTooShort`] for too few bytes, [`Error::WrongPciType`] if not an FF,
    /// [`Error::FirstFrameEscapeUnderflow`] for an escape whose `FF_DL` ≤ 4095, and
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
            let length = u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]) as usize;
            if length <= FF_DL_12BIT_MAX {
                return Err(Error::FirstFrameEscapeUnderflow(length));
            }
            FirstFrame::new(length, frame[6..].to_vec())
        } else {
            let length = (high << 8) | usize::from(frame[1]);
            if length < FF_DL_MIN {
                return Err(Error::FirstFrameLength(length));
            }
            FirstFrame::new(length, frame[2..].to_vec())
        }
    }
}

/// A `ConsecutiveFrame` (CF) N_PDU: a following frame of a segmented message (§9.6.4).
///
/// `data` holds the payload bytes as read from the frame (up to 63 on CAN FD); the last CF of a
/// message may carry fewer meaningful bytes, which the reassembler trims by `FF_DL`.
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
    /// if the payload is empty or longer than 63 bytes.
    pub fn new(sn: u8, data: Vec<u8>) -> Result<ConsecutiveFrame, Error> {
        if sn > SN_MAX {
            return Err(Error::SequenceNumberRange(sn));
        }
        if data.is_empty() || data.len() > CF_DATA_FD_MAX {
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

    /// Encode to an on-wire frame with the default padding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with(true, DEFAULT_PADDING)
    }

    /// Encode to an on-wire frame. A classic CF (≤ 7 payload bytes) honours `pad_frame`; a CAN FD
    /// CF is always padded up to the smallest valid `CAN_DL` (mandatory, §10.4.2.3).
    #[must_use]
    pub fn encode_with(&self, pad_frame: bool, padding: u8) -> Vec<u8> {
        let mut frame = vec![(PciType::ConsecutiveFrame.value() << 4) | self.sn];
        frame.extend_from_slice(&self.data);
        if frame.len() <= CLASSIC_CAN_DL {
            if pad_frame {
                pad_to(frame, CLASSIC_CAN_DL, padding)
            } else {
                frame
            }
        } else {
            let can_dl = can_dl_for(frame.len());
            pad_to(frame, can_dl, padding)
        }
    }

    /// Decode a CF frame into its `SequenceNumber` and payload bytes (padding included).
    ///
    /// # Errors
    /// [`Error::WrongPciType`] if not a CF, and [`Error::ConsecutiveFrameLength`] if it carries no
    /// data byte.
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
            pad_to(frame, CLASSIC_CAN_DL, padding)
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

/// Decode any classic-CAN or CAN FD ISO-TP frame into its N_PDU, dispatched on `N_PCItype`.
///
/// # Errors
/// Any [`Error`] the per-type decoder returns, including [`Error::ReservedPciType`] for the
/// reserved `N_PCItype` range and [`Error::InvalidCanDl`] for an invalid frame length.
pub fn decode_frame(frame: &[u8]) -> Result<NPdu, Error> {
    require_frame(frame)?;
    match PciType::from_byte1(frame[0])? {
        PciType::SingleFrame => SingleFrame::decode(frame).map(NPdu::Single),
        PciType::FirstFrame => FirstFrame::decode(frame).map(NPdu::First),
        PciType::ConsecutiveFrame => ConsecutiveFrame::decode(frame).map(NPdu::Consecutive),
        PciType::FlowControl => FlowControl::decode(frame).map(NPdu::Flow),
    }
}

/// The ISO-TP addressing format, as it affects the on-wire frame (ISO 15765-2:2016, §10.3).
///
/// `Normal` (and normal-fixed) place no byte in the data field. `Extended` puts the target
/// address `N_TA` in the first data byte, and `Mixed` puts the address extension `N_AE` there;
/// the N_PCI and N_Data follow it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Addressing {
    /// Normal (or normal-fixed) addressing: no in-band address byte (§10.3.2–§10.3.3).
    Normal,
    /// Extended addressing: `N_TA` (target address) in the first data byte (§10.3.4).
    Extended(u8),
    /// Mixed addressing: `N_AE` (address extension) in the first data byte (§10.3.5).
    Mixed(u8),
}

impl Addressing {
    /// The in-band address byte, if this format has one.
    #[must_use]
    pub const fn address_byte(self) -> Option<u8> {
        match self {
            Addressing::Normal => None,
            Addressing::Extended(byte) | Addressing::Mixed(byte) => Some(byte),
        }
    }

    /// The number of leading address bytes (0 for normal, 1 for extended/mixed).
    #[must_use]
    pub const fn header_len(self) -> usize {
        match self {
            Addressing::Normal => 0,
            Addressing::Extended(_) | Addressing::Mixed(_) => 1,
        }
    }
}

/// Encode an N_PDU to an on-wire frame for a given addressing format, with mandatory padding.
///
/// For [`Addressing::Extended`] / [`Addressing::Mixed`] the address byte leads the frame, and the
/// frame is padded up to the smallest valid `CAN_DL` that holds it. For [`Addressing::Normal`] the
/// result equals the N_PDU's own `encode`.
#[must_use]
pub fn encode_addressed(pdu: &NPdu, addressing: Addressing, padding: u8) -> Vec<u8> {
    let mut frame = Vec::new();
    if let Some(byte) = addressing.address_byte() {
        frame.push(byte);
    }
    match pdu {
        NPdu::Single(sf) => frame.extend_from_slice(&sf.encode_with(false, padding)),
        NPdu::First(ff) => frame.extend_from_slice(&ff.encode()),
        NPdu::Consecutive(cf) => frame.extend_from_slice(&cf.encode_with(false, padding)),
        NPdu::Flow(fc) => frame.extend_from_slice(&fc.encode_with(false, padding)),
    }
    let can_dl = can_dl_for(frame.len());
    pad_to(frame, can_dl, padding)
}

/// Decode an on-wire frame for a given addressing format into its N_PDU.
///
/// For [`Addressing::Extended`] / [`Addressing::Mixed`] the leading address byte is skipped (its
/// value is not matched against the format's byte — a receiver filters by CAN identifier).
///
/// # Errors
/// As [`decode_frame`], plus [`Error::EmptyFrame`] if there is no N_PCI after the address byte.
pub fn decode_addressed(frame: &[u8], addressing: Addressing) -> Result<NPdu, Error> {
    require_frame(frame)?;
    let offset = addressing.header_len();
    if offset == 0 {
        return decode_frame(frame);
    }
    let body = &frame[offset..];
    if body.is_empty() {
        return Err(Error::EmptyFrame);
    }
    let can_dl = frame.len();
    match PciType::from_byte1(body[0])? {
        PciType::SingleFrame => decode_single_frame_body(body, can_dl).map(NPdu::Single),
        PciType::FirstFrame => decode_first_frame_body(body).map(NPdu::First),
        PciType::ConsecutiveFrame => {
            if body.len() < 2 {
                return Err(Error::ConsecutiveFrameLength(0));
            }
            ConsecutiveFrame::new(body[0] & 0x0F, body[1..].to_vec()).map(NPdu::Consecutive)
        }
        PciType::FlowControl => {
            if body.len() < 3 {
                return Err(Error::FlowControlTooShort(body.len()));
            }
            let flow_status = FlowStatus::from_nibble(body[0] & 0x0F)?;
            Ok(NPdu::Flow(FlowControl::new(flow_status, body[1], body[2])))
        }
    }
}

fn decode_single_frame_body(body: &[u8], can_dl: usize) -> Result<SingleFrame, Error> {
    if can_dl <= CLASSIC_CAN_DL {
        let sf_dl = usize::from(body[0] & 0x0F);
        if sf_dl == 0 {
            return Err(Error::ReservedSingleFrame);
        }
        let available = body.len() - 1;
        if sf_dl > available {
            return Err(Error::SingleFrameLengthExceedsFrame { sf_dl, available });
        }
        SingleFrame::new(body[1..=sf_dl].to_vec())
    } else {
        if body.len() < 2 || body[0] & 0x0F != 0 {
            return Err(Error::ReservedSingleFrame);
        }
        let sf_dl = usize::from(body[1]);
        let available = body.len() - 2;
        if sf_dl <= SF_DL_MAX || sf_dl > available {
            return Err(Error::SingleFrameLengthExceedsFrame { sf_dl, available });
        }
        SingleFrame::new(body[2..2 + sf_dl].to_vec())
    }
}

fn decode_first_frame_body(body: &[u8]) -> Result<FirstFrame, Error> {
    if body.len() < 2 {
        return Err(Error::FirstFrameTooShort(body.len()));
    }
    let high = usize::from(body[0] & 0x0F);
    if high == 0 && body[1] == 0 {
        if body.len() < 6 {
            return Err(Error::FirstFrameTooShort(body.len()));
        }
        let length = u32::from_be_bytes([body[2], body[3], body[4], body[5]]) as usize;
        if length <= FF_DL_12BIT_MAX {
            return Err(Error::FirstFrameEscapeUnderflow(length));
        }
        FirstFrame::new(length, body[6..].to_vec())
    } else {
        let length = (high << 8) | usize::from(body[1]);
        if length < FF_DL_MIN_EXTENDED {
            return Err(Error::FirstFrameLength(length));
        }
        FirstFrame::new(length, body[2..].to_vec())
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
        // Classic SF_DL 0 is reserved.
        assert_eq!(
            SingleFrame::decode(&[0x00, 1]),
            Err(Error::ReservedSingleFrame)
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
    fn single_frame_can_fd_escape_roundtrip() {
        // 20-byte payload needs the CAN FD escape: SF_DL in byte #2, CAN_DL rounds up to 24.
        let payload: Vec<u8> = (0..20u8).collect();
        let sf = SingleFrame::new(payload.clone()).unwrap();
        let frame = sf.encode();
        assert_eq!(frame[0], 0x00, "byte #1 low nibble is 0 for the escape");
        assert_eq!(frame[1], 20, "SF_DL is in byte #2");
        assert_eq!(frame.len(), 24, "padded up to a valid CAN FD CAN_DL");
        assert_eq!(SingleFrame::decode(&frame).unwrap(), sf);
        // A CAN FD SF whose byte #1 low nibble is non-zero is reserved.
        let mut bad = frame.clone();
        bad[0] = 0x01;
        assert_eq!(SingleFrame::decode(&bad), Err(Error::ReservedSingleFrame));
    }

    #[test]
    fn single_frame_rejects_invalid_can_dl() {
        // 10 bytes is not a valid CAN_DL (between 8 and 12).
        assert_eq!(
            SingleFrame::decode(&[0x00, 8, 1, 2, 3, 4, 5, 6, 7, 8]),
            Err(Error::InvalidCanDl(10))
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
    }

    #[test]
    fn first_frame_32bit_escape_roundtrip() {
        // FF_DL 100_000 needs the 32-bit escape; on classic CAN the FF carries 2 data bytes.
        let ff = FirstFrame::new(100_000, vec![0xAA, 0xBB]).unwrap();
        let frame = ff.encode();
        assert_eq!(&frame[..2], &[0x10, 0x00], "escape marker");
        assert_eq!(
            u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]),
            100_000
        );
        assert_eq!(FirstFrame::decode(&frame).unwrap(), ff);
        // An escape carrying FF_DL <= 4095 must use the 12-bit form.
        assert_eq!(
            FirstFrame::decode(&[0x10, 0x00, 0, 0, 0x01, 0x00, 1, 2]),
            Err(Error::FirstFrameEscapeUnderflow(256))
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
    fn consecutive_frame_can_fd_payload() {
        // 20 payload bytes -> CAN_DL rounds up to 24 (1 PCI + 20 data = 21 -> 24).
        let data: Vec<u8> = (0..20u8).collect();
        let cf = ConsecutiveFrame::new(3, data.clone()).unwrap();
        let frame = cf.encode();
        assert_eq!(frame[0], 0x23);
        assert_eq!(frame.len(), 24);
        assert_eq!(
            ConsecutiveFrame::decode(&frame).unwrap().data()[..20],
            data[..]
        );
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
        assert_eq!(decode_frame(&[0; 65]), Err(Error::FrameTooLong(65)));
    }

    #[test]
    fn extended_addressing_prepends_the_target_address() {
        // Extended addressing: N_TA 0xF1 leads, then the SF PCI and data, padded to 8.
        let sf = NPdu::Single(SingleFrame::new(vec![0x3E, 0x00]).unwrap());
        let frame = encode_addressed(&sf, Addressing::Extended(0xF1), DEFAULT_PADDING);
        assert_eq!(frame, vec![0xF1, 0x02, 0x3E, 0x00, 0xCC, 0xCC, 0xCC, 0xCC]);
        assert_eq!(
            decode_addressed(&frame, Addressing::Extended(0xF1)).unwrap(),
            sf
        );
        // Normal addressing through the addressed codec equals the plain encode.
        assert_eq!(
            encode_addressed(&sf, Addressing::Normal, DEFAULT_PADDING),
            match &sf {
                NPdu::Single(s) => s.encode(),
                _ => unreachable!(),
            }
        );
    }

    #[test]
    fn mixed_addressing_round_trips_all_types() {
        let message_len = 40usize; // a multi-frame message's FirstFrame carries fewer bytes here
        let ff = NPdu::First(FirstFrame::new(message_len, (0..5u8).collect()).unwrap());
        let frame = encode_addressed(&ff, Addressing::Mixed(0xAA), DEFAULT_PADDING);
        assert_eq!(frame[0], 0xAA, "N_AE leads");
        assert_eq!(frame.len(), 8, "address + 2 PCI + 5 data = 8");
        assert_eq!(
            decode_addressed(&frame, Addressing::Mixed(0xAA)).unwrap(),
            ff
        );

        let cf = NPdu::Consecutive(ConsecutiveFrame::new(1, (0..6u8).collect()).unwrap());
        let cframe = encode_addressed(&cf, Addressing::Mixed(0xAA), DEFAULT_PADDING);
        assert_eq!(
            decode_addressed(&cframe, Addressing::Mixed(0xAA)).unwrap(),
            cf
        );

        let fc = NPdu::Flow(FlowControl::continue_to_send());
        let fcframe = encode_addressed(&fc, Addressing::Extended(0x55), DEFAULT_PADDING);
        assert_eq!(fcframe[..4], [0x55, 0x30, 0, 0]);
        assert_eq!(
            decode_addressed(&fcframe, Addressing::Extended(0x55)).unwrap(),
            fc
        );
    }
}
