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
    /// A frame longer than the classic-CAN 8-byte limit (CAN FD is a planned row).
    FrameTooLong(usize),
    /// The high nibble of N_PCI byte #1 is in the reserved 0x4..=0xF range (§9.6.1, Table 8).
    ReservedPciType(u8),
    /// A decoder was asked to read a frame whose `N_PCItype` is a different N_PDU.
    WrongPciType {
        /// The N_PDU type the decoder expected.
        expected: PciType,
        /// The actual N_PCI byte #1.
        byte1: u8,
    },
    /// `SingleFrame` payload length (`SF_DL`) is outside the classic-CAN 1..=7 range.
    SingleFrameLength(usize),
    /// A decoded `SingleFrame` has `SF_DL` 0, the CAN FD escape (planned row) (§9.6.2).
    SingleFrameEscape,
    /// `SF_DL` is larger than the data bytes the frame actually carries.
    SingleFrameLengthExceedsFrame {
        /// The `SF_DL` read from the frame.
        sf_dl: usize,
        /// The data bytes available after N_PCI byte #1.
        available: usize,
    },
    /// A `FirstFrame` frame has fewer than the required 8 bytes (§9.6.3).
    FirstFrameTooShort(usize),
    /// A decoded `FirstFrame` is the 32-bit `FF_DL` escape, `FF_DL` > 4095 (planned row) (§9.6.3).
    FirstFrameEscape,
    /// `FirstFrame` length (`FF_DL`) is outside the classic-CAN 8..=4095 range.
    FirstFrameLength(usize),
    /// A `FirstFrame` was built with other than the 6 payload bytes it carries.
    FirstFrameDataLength(usize),
    /// A `SequenceNumber` is outside the 0..=15 range (§9.6.4).
    SequenceNumberRange(u8),
    /// A `ConsecutiveFrame` payload is outside the classic-CAN 1..=7 range.
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
    /// An empty application message was given to the segmenter.
    EmptyMessage,
    /// A message longer than the 12-bit `FF_DL` maximum of 4095 bytes (32-bit escape is planned).
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyFrame => f.write_str("empty CAN frame"),
            Error::FrameTooLong(len) => {
                write!(
                    f,
                    "classic CAN frame is at most 8 bytes, got {len}; CAN FD is a planned row"
                )
            }
            Error::ReservedPciType(nibble) => {
                write!(
                    f,
                    "reserved N_PCItype {nibble:#x} (ISO 15765-2:2016, 9.6.1, Table 8)"
                )
            }
            Error::WrongPciType { expected, byte1 } => {
                write!(f, "not a {expected:?}: N_PCI byte #1 is {byte1:#04x}")
            }
            Error::SingleFrameLength(len) => {
                write!(f, "SingleFrame payload must be 1..=7 bytes, got {len}")
            }
            Error::SingleFrameEscape => {
                f.write_str("SF_DL 0 is the CAN FD SingleFrame escape (SF_DL > 7), a planned row")
            }
            Error::SingleFrameLengthExceedsFrame { sf_dl, available } => {
                write!(
                    f,
                    "SF_DL {sf_dl} exceeds the {available} available data bytes"
                )
            }
            Error::FirstFrameTooShort(len) => {
                write!(f, "FirstFrame requires an 8-byte frame (9.6.3), got {len}")
            }
            Error::FirstFrameEscape => {
                f.write_str("FirstFrame escape sequence (FF_DL > 4095, 32-bit) is a planned row")
            }
            Error::FirstFrameLength(len) => {
                write!(f, "FF_DL must be 8..=4095, got {len}")
            }
            Error::FirstFrameDataLength(len) => {
                write!(f, "FirstFrame carries 6 payload bytes, got {len}")
            }
            Error::SequenceNumberRange(sn) => {
                write!(f, "SequenceNumber must be 0..=15, got {sn}")
            }
            Error::ConsecutiveFrameLength(len) => {
                write!(f, "ConsecutiveFrame payload must be 1..=7 bytes, got {len}")
            }
            Error::FlowControlTooShort(len) => {
                write!(f, "FlowControl needs 3 N_PCI bytes, got {len}")
            }
            Error::ReservedFlowStatus(nibble) => {
                write!(f, "reserved FlowStatus {nibble:#x} (N_INVALID_FS, 9.6.5)")
            }
            Error::StminNotRepresentable(us) => {
                write!(
                    f,
                    "STmin {us} us is not representable in ISO 15765-2 STmin (9.6.5)"
                )
            }
            Error::CanIdRange { label, value } => {
                write!(
                    f,
                    "{label} {value:#x} is outside the 11-bit range 0..=0x7ff"
                )
            }
            Error::EmptyMessage => f.write_str("cannot segment an empty message"),
            Error::MessageTooLong(len) => {
                write!(
                    f,
                    "message of {len} bytes exceeds the 12-bit FF_DL max 4095; the 32-bit escape is a planned row"
                )
            }
            Error::WrongSequenceNumber { expected, got } => {
                write!(
                    f,
                    "wrong SequenceNumber: expected {expected}, got {got} (N_WRONG_SN, 9.6.4)"
                )
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
        }
    }
}

impl core::error::Error for Error {}
