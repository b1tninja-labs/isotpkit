//! The `<N_Result>` outcome enumeration of the network-layer service primitives (§8.3.7).
//!
//! ISO 15765-2:2016 models every `N_USData` service outcome as one `<N_Result>` value. This crate
//! is sans-I/O and owns no timer, so it does not *run* the `N_USData` primitives; [`NResult`] is the
//! grounded enumeration those primitives report, together with the two rules §8.3.7 states about it:
//!
//! * the **priority ordering** — "if two or more errors are discovered at the same time, then the
//!   network layer entity shall use the parameter value found first in this list" — exposed as
//!   [`NResult::rank`] and [`NResult::first_in_list`];
//! * the **side** each value can be issued on (sender, receiver, or both) — [`NResult::side`].
//!
//! The crate's own [`Error`] is a decode/encode error type, not this model; `&Error`
//! maps onto its `<N_Result>` value through [`NResult::from`]. The three timeout codes
//! (`N_TIMEOUT_A`/`Bs`/`Cr`) have no [`Error`] counterpart because the crate runs no
//! timer (§9.8.1) — the caller that owns the timers reports them.

use crate::error::Error;

/// Which side of a transfer a `<N_Result>` value can be issued to (§8.3.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Issued on the sender side only.
    Sender,
    /// Issued on the receiver side only.
    Receiver,
    /// Issued on both the sender and the receiver side.
    Both,
}

/// The `<N_Result>` enumeration (§8.3.7, in the standard's list order).
///
/// The discriminants follow the order the standard lists the values in, which is also the priority
/// order: lower-ranked values are "found first in this list" and win when several errors are
/// discovered at once (see [`NResult::first_in_list`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NResult {
    /// `N_OK`: the service completed successfully (sender and receiver).
    Ok,
    /// `N_TIMEOUT_A`: `N_Ar`/`N_As` timed out (sender and receiver).
    TimeoutA,
    /// `N_TIMEOUT_Bs`: `N_Bs` timed out (sender only).
    TimeoutBs,
    /// `N_TIMEOUT_Cr`: `N_Cr` timed out (receiver only).
    TimeoutCr,
    /// `N_WRONG_SN`: an unexpected `SequenceNumber` was received (receiver only).
    WrongSn,
    /// `N_INVALID_FS`: an invalid or unknown `FlowStatus` was received (sender only).
    InvalidFs,
    /// `N_UNEXP_PDU`: an unexpected N_PDU was received (receiver only).
    UnexpPdu,
    /// `N_WFT_OVRN`: `N_WFTmax` FC.WAIT frames were sent in a row (receiver only).
    WftOvrn,
    /// `N_BUFFER_OVFLW`: a FC with `FlowStatus` = OVFLW was received (sender only).
    BufferOvflw,
    /// `N_ERROR`: the general error value, when no other value describes it better (both sides).
    Error,
}

impl NResult {
    /// The value's position in the §8.3.7 list (0 = `N_OK`), which is its priority rank.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            NResult::Ok => 0,
            NResult::TimeoutA => 1,
            NResult::TimeoutBs => 2,
            NResult::TimeoutCr => 3,
            NResult::WrongSn => 4,
            NResult::InvalidFs => 5,
            NResult::UnexpPdu => 6,
            NResult::WftOvrn => 7,
            NResult::BufferOvflw => 8,
            NResult::Error => 9,
        }
    }

    /// The canonical `N_…` name of the value.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            NResult::Ok => "N_OK",
            NResult::TimeoutA => "N_TIMEOUT_A",
            NResult::TimeoutBs => "N_TIMEOUT_Bs",
            NResult::TimeoutCr => "N_TIMEOUT_Cr",
            NResult::WrongSn => "N_WRONG_SN",
            NResult::InvalidFs => "N_INVALID_FS",
            NResult::UnexpPdu => "N_UNEXP_PDU",
            NResult::WftOvrn => "N_WFT_OVRN",
            NResult::BufferOvflw => "N_BUFFER_OVFLW",
            NResult::Error => "N_ERROR",
        }
    }

    /// Which side the value can be issued to (§8.3.7).
    #[must_use]
    pub const fn side(self) -> Side {
        match self {
            NResult::Ok | NResult::TimeoutA | NResult::Error => Side::Both,
            NResult::TimeoutBs | NResult::InvalidFs | NResult::BufferOvflw => Side::Sender,
            NResult::TimeoutCr | NResult::WrongSn | NResult::UnexpPdu | NResult::WftOvrn => {
                Side::Receiver
            }
        }
    }

    /// Whether the value denotes an error (anything other than `N_OK`).
    #[must_use]
    pub const fn is_error(self) -> bool {
        !matches!(self, NResult::Ok)
    }

    /// The value "found first in this list" of the two — the §8.3.7 priority rule for two or more
    /// errors discovered at the same time. It is the one with the lower [`rank`](NResult::rank).
    #[must_use]
    pub const fn first_in_list(self, other: NResult) -> NResult {
        if other.rank() < self.rank() {
            other
        } else {
            self
        }
    }

    /// The highest-priority value across the `results`, applying the §8.3.7 rule, or `None` when the
    /// slice is empty.
    #[must_use]
    pub fn earliest(results: &[NResult]) -> Option<NResult> {
        results.iter().copied().reduce(NResult::first_in_list)
    }
}

impl From<&Error> for NResult {
    /// Map a crate [`Error`] onto the `<N_Result>` value it represents. Conditions the
    /// standard names a specific code for map to it; anything else is the general `N_ERROR` (§8.3.7).
    fn from(error: &Error) -> NResult {
        match error {
            Error::BufferOverflow => NResult::BufferOvflw,
            Error::WaitFrameOverrun(_) => NResult::WftOvrn,
            Error::WrongSequenceNumber { .. } => NResult::WrongSn,
            Error::ReservedFlowStatus(_) => NResult::InvalidFs,
            // An N_PDU that arrives out of turn is an "unexpected protocol data unit" (§9.8.3).
            Error::SingleFrameDuringReassembly
            | Error::FirstFrameDuringReassembly
            | Error::ConsecutiveFrameWithoutFirstFrame => NResult::UnexpPdu,
            // No better-fitting value: the general error (§8.3.7).
            _ => NResult::Error,
        }
    }
}

impl From<Error> for NResult {
    fn from(error: Error) -> NResult {
        NResult::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_follow_the_list_order() {
        // The exact §8.3.7 order, N_OK first through N_ERROR last.
        let order = [
            NResult::Ok,
            NResult::TimeoutA,
            NResult::TimeoutBs,
            NResult::TimeoutCr,
            NResult::WrongSn,
            NResult::InvalidFs,
            NResult::UnexpPdu,
            NResult::WftOvrn,
            NResult::BufferOvflw,
            NResult::Error,
        ];
        for (i, r) in order.iter().enumerate() {
            assert_eq!(r.rank() as usize, i, "{}", r.name());
        }
    }

    #[test]
    fn first_in_list_takes_the_earlier_value() {
        // Two errors discovered at once: the one earlier in the list wins (§8.3.7).
        assert_eq!(
            NResult::BufferOvflw.first_in_list(NResult::WrongSn),
            NResult::WrongSn
        );
        // Order-independent.
        assert_eq!(
            NResult::WrongSn.first_in_list(NResult::BufferOvflw),
            NResult::WrongSn
        );
        // N_ERROR is the general catch-all and loses to every named error.
        assert_eq!(
            NResult::Error.first_in_list(NResult::WftOvrn),
            NResult::WftOvrn
        );
    }

    #[test]
    fn earliest_reduces_a_set_of_simultaneous_errors() {
        let discovered = [NResult::Error, NResult::BufferOvflw, NResult::InvalidFs];
        assert_eq!(NResult::earliest(&discovered), Some(NResult::InvalidFs));
        assert_eq!(NResult::earliest(&[]), None);
    }

    #[test]
    fn sides_match_the_standard() {
        assert_eq!(NResult::Ok.side(), Side::Both);
        assert_eq!(NResult::Error.side(), Side::Both);
        assert_eq!(NResult::TimeoutA.side(), Side::Both);
        assert_eq!(NResult::BufferOvflw.side(), Side::Sender); // sender only
        assert_eq!(NResult::InvalidFs.side(), Side::Sender); // sender only
        assert_eq!(NResult::TimeoutBs.side(), Side::Sender); // sender only
        assert_eq!(NResult::WrongSn.side(), Side::Receiver); // receiver only
        assert_eq!(NResult::UnexpPdu.side(), Side::Receiver); // receiver only
        assert_eq!(NResult::WftOvrn.side(), Side::Receiver); // receiver only
        assert_eq!(NResult::TimeoutCr.side(), Side::Receiver); // receiver only
    }

    #[test]
    fn errors_map_to_their_result_codes() {
        assert_eq!(NResult::from(&Error::BufferOverflow), NResult::BufferOvflw);
        assert_eq!(NResult::from(&Error::WaitFrameOverrun(3)), NResult::WftOvrn);
        assert_eq!(
            NResult::from(&Error::WrongSequenceNumber {
                expected: 1,
                got: 2
            }),
            NResult::WrongSn
        );
        assert_eq!(
            NResult::from(&Error::ReservedFlowStatus(0xF)),
            NResult::InvalidFs
        );
        assert_eq!(
            NResult::from(&Error::FirstFrameDuringReassembly),
            NResult::UnexpPdu
        );
        // A plain frame-shape error has no dedicated code: N_ERROR.
        assert_eq!(NResult::from(&Error::EmptyFrame), NResult::Error);
        assert!(NResult::from(&Error::EmptyFrame).is_error());
        assert!(!NResult::Ok.is_error());
    }
}
