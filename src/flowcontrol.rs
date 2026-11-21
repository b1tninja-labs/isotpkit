//! `FlowControl` parameters: `FlowStatus`, `BlockSize` and the `STmin` encoding.
//!
//! Grounded in ISO 15765-2:2016, §9.6.5 (`FlowControl` N_PCI parameter definition).

use crate::error::Error;

/// `FlowStatus` (FS), the low nibble of the FC N_PCI byte (§9.6.5, Table 18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowStatus {
    /// CTS (0x0): the sender may send up to `BS` `ConsecutiveFrames`.
    ContinueToSend,
    /// WAIT (0x1): the sender keeps waiting for a new FC and restarts `N_Bs`; `BS`/`STmin` ignored.
    Wait,
    /// OVFLW (0x2): the receiver buffer is too small for `FF_DL`; the sender aborts (`N_BUFFER_OVFLW`).
    Overflow,
}

impl FlowStatus {
    /// The 4-bit `FlowStatus` value.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            FlowStatus::ContinueToSend => 0x0,
            FlowStatus::Wait => 0x1,
            FlowStatus::Overflow => 0x2,
        }
    }

    /// The short name used in diagnostics output (CTS, WAIT, OVFLW).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            FlowStatus::ContinueToSend => "CTS",
            FlowStatus::Wait => "WAIT",
            FlowStatus::Overflow => "OVFLW",
        }
    }

    /// Parse the low nibble of the FC N_PCI byte.
    ///
    /// # Errors
    /// [`Error::ReservedFlowStatus`] for the reserved 0x3..=0xF range (`N_INVALID_FS`, §9.6.5).
    pub const fn from_nibble(nibble: u8) -> Result<FlowStatus, Error> {
        match nibble {
            0x0 => Ok(FlowStatus::ContinueToSend),
            0x1 => Ok(FlowStatus::Wait),
            0x2 => Ok(FlowStatus::Overflow),
            other => Err(Error::ReservedFlowStatus(other)),
        }
    }
}

/// A reserved `STmin` byte is treated as the longest value, 0x7F = 127 ms (§9.6.5).
pub const RESERVED_STMIN_FALLBACK_US: u32 = 127_000;

const STMIN_MS_MAX: u8 = 0x7F; // 0x00..=0x7F are 0..=127 ms
const STMIN_US_LOW: u8 = 0xF1; // 0xF1..=0xF9 are 100..=900 us
const STMIN_US_HIGH: u8 = 0xF9;

/// A parsed `STmin` byte (§9.6.5, Table 20).
///
/// `raw` is the on-wire byte, `microseconds` the resolved minimum separation time, and
/// `reserved` whether `raw` fell in a reserved range (then `microseconds` is the 127 ms fallback
/// the sender must use per §9.6.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeparationTime {
    raw: u8,
    microseconds: u32,
    reserved: bool,
}

impl SeparationTime {
    /// Parse one `STmin` byte into its minimum-separation time in microseconds.
    #[must_use]
    pub fn from_byte(raw: u8) -> SeparationTime {
        match raw {
            0x00..=STMIN_MS_MAX => SeparationTime {
                raw,
                microseconds: u32::from(raw) * 1000,
                reserved: false,
            },
            STMIN_US_LOW..=STMIN_US_HIGH => SeparationTime {
                raw,
                microseconds: u32::from(raw - 0xF0) * 100,
                reserved: false,
            },
            // 0x80..=0xF0 and 0xFA..=0xFF are reserved.
            _ => SeparationTime {
                raw,
                microseconds: RESERVED_STMIN_FALLBACK_US,
                reserved: true,
            },
        }
    }

    /// The on-wire `STmin` byte.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        self.raw
    }

    /// The resolved minimum separation time, in microseconds.
    #[must_use]
    pub const fn microseconds(self) -> u32 {
        self.microseconds
    }

    /// Whether the raw byte was in a reserved range (so `microseconds` is the 127 ms fallback).
    #[must_use]
    pub const fn reserved(self) -> bool {
        self.reserved
    }
}

/// Encode a minimum separation time in microseconds into an `STmin` byte.
///
/// Whole milliseconds 0..=127 use the 0x00..=0x7F range; 100..=900 µs in 100 µs steps use the
/// 0xF1..=0xF9 range.
///
/// # Errors
/// [`Error::StminNotRepresentable`] for any other value (§9.6.5, Table 20).
#[expect(
    clippy::cast_possible_truncation,
    reason = "each branch has already bounded the quotient to a u8 range"
)]
pub const fn encode_stmin(microseconds: u32) -> Result<u8, Error> {
    if microseconds % 1000 == 0 && microseconds / 1000 <= 127 {
        return Ok((microseconds / 1000) as u8);
    }
    if matches!(microseconds, 100..=900) && microseconds % 100 == 0 {
        return Ok(0xF0 + (microseconds / 100) as u8);
    }
    Err(Error::StminNotRepresentable(microseconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stmin_milliseconds() {
        assert_eq!(SeparationTime::from_byte(0x00).microseconds(), 0);
        assert_eq!(SeparationTime::from_byte(0x0A).microseconds(), 10_000);
        assert_eq!(SeparationTime::from_byte(0x7F).microseconds(), 127_000);
        assert!(!SeparationTime::from_byte(0x7F).reserved());
    }

    #[test]
    fn stmin_microseconds() {
        assert_eq!(SeparationTime::from_byte(0xF1).microseconds(), 100);
        assert_eq!(SeparationTime::from_byte(0xF5).microseconds(), 500);
        assert_eq!(SeparationTime::from_byte(0xF9).microseconds(), 900);
    }

    #[test]
    fn reserved_stmin_falls_back() {
        for raw in [0x80, 0xC0, 0xF0, 0xFA, 0xFF] {
            let st = SeparationTime::from_byte(raw);
            assert!(st.reserved());
            assert_eq!(st.microseconds(), RESERVED_STMIN_FALLBACK_US);
        }
    }

    #[test]
    fn encode_stmin_roundtrip_and_rejects() {
        assert_eq!(encode_stmin(10_000), Ok(0x0A));
        assert_eq!(encode_stmin(127_000), Ok(0x7F));
        assert_eq!(encode_stmin(100), Ok(0xF1));
        assert_eq!(encode_stmin(900), Ok(0xF9));
        assert_eq!(encode_stmin(50), Err(Error::StminNotRepresentable(50)));
        assert_eq!(
            encode_stmin(128_000),
            Err(Error::StminNotRepresentable(128_000))
        );
    }
}
