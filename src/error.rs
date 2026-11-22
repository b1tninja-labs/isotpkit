//! The crate's error type: every way a frame, message or parameter can be rejected.

use core::fmt;

use crate::pci::PciType;

/// An ISO-TP encode/decode error.
///
/// Each variant names the ISO 15765-2:2016 condition it comes from where the standard defines
/// one (for example `N_WRONG_SN` and `N_INVALID_FS`); `docs/iso-map.md` has the clause map.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A CAN frame with no bytes was given to a decoder.
    EmptyFrame,
    /// A frame longer than the CAN FD 64-byte maximum (§9.5.4).
    FrameTooLong(usize),
    /// A frame whose `CAN_DL` is not a valid data length — 1..=8, or a CAN FD 12/16/20/24/32/48/64 (§9.5.4, Table 7).
    InvalidCanDl(usize),
    /// The high nibble of N_PCI byte #1 is in the reserved 0x4..=0xF range (§9.6.1, Table 8).
    ReservedPciType(u8),
    /// A decoder was asked to read a frame whose `N_PCItype` is a different N_PDU.
    WrongPciType {
        /// The N_PDU type the decoder expected.
        expected: PciType,
        /// The actual N_PCI byte #1.
        byte1: u8,
    },
    /// `SingleFrame` payload length (`SF_DL`) is outside the 1..=62 range.
    SingleFrameLength(usize),
    /// A reserved `SingleFrame`: `SF_DL` 0 on classic CAN, or a non-zero low nibble on a CAN FD escape (§9.6.2).
    ReservedSingleFrame,
    /// `SF_DL` is larger than the data bytes the frame actually carries, or below the CAN FD minimum.
    SingleFrameLengthExceedsFrame {
        /// The `SF_DL` read from the frame.
        sf_dl: usize,
        /// The data bytes available after the N_PCI byte(s).
        available: usize,
    },
    /// A `FirstFrame` frame has fewer N_PCI bytes than its form requires (§9.6.3).
    FirstFrameTooShort(usize),
    /// A `FirstFrame` 32-bit escape carries an `FF_DL` ≤ 4095, which must use the 12-bit form (§9.6.3).
    FirstFrameEscapeUnderflow(usize),
    /// `FirstFrame` length (`FF_DL`) is outside the 8..=4294967295 range.
    FirstFrameLength(usize),
    /// A `FirstFrame` was built with a payload that does not fill a valid `CAN_DL`.
    FirstFrameDataLength(usize),
    /// A `SequenceNumber` is outside the 0..=15 range (§9.6.4).
    SequenceNumberRange(u8),
    /// A `ConsecutiveFrame` payload is outside the 1..=63 range.
    ConsecutiveFrameLength(usize),
    /// A `FlowControl` frame has fewer than its 3 N_PCI bytes (§9.6.5).
    FlowControlTooShort(usize),
    /// A decoded `FlowControl` has a reserved `FlowStatus` (`N_INVALID_FS`) (§9.6.5).
    ReservedFlowStatus(u8),
    /// A separation time in microseconds is not representable as an `STmin` byte (§9.6.5).
    StminNotRepresentable(u32),
    /// A CAN identifier is outside the 11-bit 0..=0x7FF range (§10.3.2).
    CanIdRange {
        /// Which identifier was out of range.
        label: &'static str,
        /// The offending value.
        value: u32,
    },
    /// A CAN identifier falls in an ISO-reserved 11-bit range, 0x7F4..=0x7F6 or 0x7FA..=0x7FB (Annex B).
    ReservedCanId {
        /// Which identifier was reserved.
        label: &'static str,
        /// The offending value.
        value: u32,
    },
    /// An empty application message was given to the segmenter.
    EmptyMessage,
    /// A message longer than the 32-bit `FF_DL` maximum of 4294967295 bytes.
    MessageTooLong(usize),
    /// A `ConsecutiveFrame` arrived with the wrong `SequenceNumber` (`N_WRONG_SN`) (§9.6.4).
    WrongSequenceNumber {
        /// The `SequenceNumber` the reassembler expected next.
        expected: u8,
        /// The `SequenceNumber` actually received.
        got: u8,
    },
    /// A `SingleFrame` arrived while a segmented message was being reassembled.
    SingleFrameDuringReassembly,
    /// A `FirstFrame` arrived while a segmented message was already being reassembled.
    FirstFrameDuringReassembly,
    /// A `ConsecutiveFrame` arrived before any `FirstFrame` opened a message.
    ConsecutiveFrameWithoutFirstFrame,
    /// An N_PDU sequence ended without completing a message.
    IncompleteSequence,
    /// A `FlowControl` Overflow aborted the send (`N_BUFFER_OVFLW`) (§9.6.5).
    BufferOverflow,
    /// The receiver sent `N_WFTmax` FC.WAIT frames in a row (`N_WFT_OVRN`) (§9.7, §9.8.4).
    WaitFrameOverrun(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyFrame => f.write_str("empty CAN frame"),
            Error::FrameTooLong(len) => {
                write!(f, "CAN frame is at most 64 bytes (CAN FD), got {len}")
            }
            Error::InvalidCanDl(len) => {
                write!(f, "invalid CAN_DL {len}: not 1..=8 or a CAN FD 12/16/20/24/32/48/64 (9.5.4)")
            }
            Error::ReservedPciType(nibble) => {
                write!(f, "reserved N_PCItype {nibble:#x} (ISO 15765-2:2016, 9.6.1, Table 8)")
            }
            Error::WrongPciType { expected, byte1 } => {
                write!(f, "not a {expected:?}: N_PCI byte #1 is {byte1:#04x}")
            }
            Error::SingleFrameLength(len) => {
                write!(f, "SingleFrame payload must be 1..=62 bytes, got {len}")
            }
            Error::ReservedSingleFrame => {
                f.write_str("reserved SingleFrame: SF_DL 0 on classic CAN, or a non-zero low nibble on a CAN FD escape (9.6.2)")
            }
            Error::SingleFrameLengthExceedsFrame { sf_dl, available } => {
                write!(f, "SF_DL {sf_dl} does not fit the {available} available data bytes")
            }
            Error::FirstFrameTooShort(len) => {
                write!(f, "FirstFrame has too few N_PCI bytes for its form (9.6.3), got {len}")
            }
            Error::FirstFrameEscapeUnderflow(len) => {
                write!(f, "FirstFrame 32-bit escape FF_DL {len} <= 4095 must use the 12-bit form (9.6.3)")
            }
            Error::FirstFrameLength(len) => {
                write!(f, "FF_DL must be 8..=4294967295, got {len}")
            }
            Error::FirstFrameDataLength(len) => {
                write!(f, "FirstFrame payload of {len} bytes does not fill a valid CAN_DL")
            }
            Error::SequenceNumberRange(sn) => {
                write!(f, "SequenceNumber must be 0..=15, got {sn}")
            }
            Error::ConsecutiveFrameLength(len) => {
                write!(f, "ConsecutiveFrame payload must be 1..=63 bytes, got {len}")
            }
            Error::FlowControlTooShort(len) => {
                write!(f, "FlowControl needs 3 N_PCI bytes, got {len}")
            }
            Error::ReservedFlowStatus(nibble) => {
                write!(f, "reserved FlowStatus {nibble:#x} (N_INVALID_FS, 9.6.5)")
            }
            Error::StminNotRepresentable(us) => {
                write!(f, "STmin {us} us is not representable in ISO 15765-2 STmin (9.6.5)")
            }
            Error::CanIdRange { label, value } => {
                write!(f, "{label} {value:#x} is outside the 11-bit range 0..=0x7ff")
            }
            Error::ReservedCanId { label, value } => {
                write!(f, "{label} {value:#x} is an ISO-reserved CAN ID (0x7f4..=0x7f6 or 0x7fa..=0x7fb, Annex B)")
            }
            Error::EmptyMessage => f.write_str("cannot segment an empty message"),
            Error::MessageTooLong(len) => {
                write!(f, "message of {len} bytes exceeds the 32-bit FF_DL max 4294967295")
            }
            Error::WrongSequenceNumber { expected, got } => {
                write!(f, "wrong SequenceNumber: expected {expected}, got {got} (N_WRONG_SN, 9.6.4)")
            }
            Error::SingleFrameDuringReassembly => {
                f.write_str("SingleFrame arrived during an in-progress reassembly")
            }
            Error::FirstFrameDuringReassembly => {
                f.write_str("FirstFrame arrived during an in-progress reassembly")
            }
            Error::ConsecutiveFrameWithoutFirstFrame => {
                f.write_str("ConsecutiveFrame arrived before any FirstFrame")
            }
            Error::IncompleteSequence => {
                f.write_str("the N_PDU sequence did not complete a message")
            }
            Error::BufferOverflow => {
                f.write_str("the send was aborted by a FlowControl Overflow (N_BUFFER_OVFLW, 9.6.5)")
            }
            Error::WaitFrameOverrun(max) => {
                write!(f, "the receiver sent {max} FC.WAIT frames in a row (N_WFT_OVRN, 9.7)")
            }
        }
    }
}

impl core::error::Error for Error {}
