//! 11-bit normal addressing, and the GM GMLAN request/response/filter offset convention.
//!
//! Normal addressing (ISO 15765-2:2016, §10.3.2): the whole `N_AI` is the CAN identifier, so the
//! CAN data field is N_PCI + N_Data with no in-band address byte. Extended and mixed addressing
//! (§10.3.4, §10.3.5) place an address byte (N_TA or N_AE) in the first data byte — handled by
//! [`crate::pci::Addressing`] and the addressed codec. Normal-fixed (§10.3.3) and 29-bit mixed
//! addressing map `N_SA`/`N_TA` into a 29-bit CAN identifier; the helpers below build those IDs.
//!
//! The GMLAN 11-bit offsets are documented, read-only, in the `volt` research notes
//! (`docs/OBD-CONFIG-DID-APPENDIX.md`): a GM physical response ID is the request ID + 0x400, and
//! the flow/acceptance filter ID is the request ID + 0x300.

use crate::error::Error;

/// The largest 11-bit CAN identifier (0x7FF).
pub const CAN_11BIT_MAX: u32 = 0x7FF;
/// The largest 29-bit CAN identifier (0x1FFFFFFF).
pub const CAN_29BIT_MAX: u32 = 0x1FFF_FFFF;

/// The lower ISO-reserved 11-bit CAN ID range, 0x7F4..=0x7F6 (Annex B, Table B.1).
pub const RESERVED_CAN_IDS_LOW: core::ops::RangeInclusive<u32> = 0x7F4..=0x7F6;
/// The upper ISO-reserved 11-bit CAN ID range, 0x7FA..=0x7FB (Annex B, Table B.1).
pub const RESERVED_CAN_IDS_HIGH: core::ops::RangeInclusive<u32> = 0x7FA..=0x7FB;

/// Whether `value` is an ISO 15765-2-reserved 11-bit CAN ID: 0x7F4..=0x7F6 or 0x7FA..=0x7FB, which
/// Annex B reserves for future use in International Standards and a conformant node must not use.
#[must_use]
pub const fn is_reserved_can_id(value: u32) -> bool {
    matches!(value, 0x7F4..=0x7F6 | 0x7FA..=0x7FB)
}

// ISO 15765-2 29-bit addressing (§10.3.3, §10.3.5): priority 110b in bits 28-26, bits 25-24 = 0,
// then an 8-bit "PF" selector in bits 23-16, N_TA in 15-8, N_SA in 7-0.
const PRIORITY_BASE_29BIT: u32 = 0x1800_0000;
/// Normal-fixed addressing PF selector, physical (§10.3.3, Table 26): 0xDA.
pub const NORMAL_FIXED_PHYSICAL_PF: u32 = 0xDA;
/// Normal-fixed addressing PF selector, functional (§10.3.3, Table 27): 0xDB.
pub const NORMAL_FIXED_FUNCTIONAL_PF: u32 = 0xDB;
/// Mixed addressing PF selector, physical (§10.3.5, Table 30): 0xCE.
pub const MIXED_PHYSICAL_PF: u32 = 0xCE;
/// Mixed addressing PF selector, functional (§10.3.5, Table 31): 0xCD.
pub const MIXED_FUNCTIONAL_PF: u32 = 0xCD;

#[allow(
    clippy::similar_names,
    reason = "N_TA and N_SA are ISO 15765-2's own parameter names"
)]
fn id_29bit(pf: u32, n_ta: u8, n_sa: u8) -> u32 {
    PRIORITY_BASE_29BIT | (pf << 16) | (u32::from(n_ta) << 8) | u32::from(n_sa)
}

/// The normal-fixed 29-bit CAN identifier for a target/source pair (§10.3.3): `0x18DA<TA><SA>`
/// physical, `0x18DB<TA><SA>` functional.
#[must_use]
#[allow(
    clippy::similar_names,
    reason = "N_TA and N_SA are ISO 15765-2's own parameter names"
)]
pub fn normal_fixed_id(n_ta: u8, n_sa: u8, functional: bool) -> u32 {
    let pf = if functional {
        NORMAL_FIXED_FUNCTIONAL_PF
    } else {
        NORMAL_FIXED_PHYSICAL_PF
    };
    id_29bit(pf, n_ta, n_sa)
}

/// The mixed-addressing 29-bit CAN identifier for a target/source pair (§10.3.5): `0x18CE<TA><SA>`
/// physical, `0x18CD<TA><SA>` functional. The `N_AE` byte travels in the first data byte, not here.
#[must_use]
#[allow(
    clippy::similar_names,
    reason = "N_TA and N_SA are ISO 15765-2's own parameter names"
)]
pub fn mixed_id_29bit(n_ta: u8, n_sa: u8, functional: bool) -> u32 {
    let pf = if functional {
        MIXED_FUNCTIONAL_PF
    } else {
        MIXED_PHYSICAL_PF
    };
    id_29bit(pf, n_ta, n_sa)
}
/// The GM GMLAN physical response offset (request + 0x400).
pub const GMLAN_RESPONSE_OFFSET: u32 = 0x400;
/// The GM GMLAN flow/acceptance filter offset (request + 0x300).
pub const GMLAN_FILTER_OFFSET: u32 = 0x300;

fn check_11bit(value: u32, label: &'static str) -> Result<(), Error> {
    if value > CAN_11BIT_MAX {
        return Err(Error::CanIdRange { label, value });
    }
    // Annex B reserves these 11-bit IDs for future use; a conformant node must not use one.
    if is_reserved_can_id(value) {
        return Err(Error::ReservedCanId { label, value });
    }
    Ok(())
}

/// A normal-addressing 11-bit request/response CAN identifier pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalAddressing {
    request_id: u32,
    response_id: u32,
}

impl NormalAddressing {
    /// Build a request/response pair, rejecting an identifier wider than 11 bits or in an
    /// ISO-reserved range (Annex B).
    ///
    /// # Errors
    /// [`Error::CanIdRange`] if either identifier is above 0x7FF, or [`Error::ReservedCanId`] if
    /// either is in an ISO-reserved range (0x7F4..=0x7F6 or 0x7FA..=0x7FB).
    pub fn new(request_id: u32, response_id: u32) -> Result<NormalAddressing, Error> {
        check_11bit(request_id, "request_id")?;
        check_11bit(response_id, "response_id")?;
        Ok(NormalAddressing {
            request_id,
            response_id,
        })
    }

    /// The request (tester → ECU) CAN identifier.
    #[must_use]
    pub const fn request_id(self) -> u32 {
        self.request_id
    }

    /// The response (ECU → tester) CAN identifier.
    #[must_use]
    pub const fn response_id(self) -> u32 {
        self.response_id
    }
}

/// The GM GMLAN physical response ID for a request ID (request + 0x400).
///
/// # Errors
/// [`Error::CanIdRange`] if the request ID or the resulting response ID is above 0x7FF, or
/// [`Error::ReservedCanId`] if either is in an ISO-reserved range (Annex B).
pub fn gmlan_response_id(request_id: u32) -> Result<u32, Error> {
    check_11bit(request_id, "request_id")?;
    let response_id = request_id + GMLAN_RESPONSE_OFFSET;
    check_11bit(response_id, "response_id")?;
    Ok(response_id)
}

/// The GM GMLAN flow/acceptance filter ID for a request ID (request + 0x300).
///
/// # Errors
/// [`Error::CanIdRange`] if the request ID or the resulting filter ID is above 0x7FF, or
/// [`Error::ReservedCanId`] if either is in an ISO-reserved range (Annex B).
pub fn gmlan_filter_id(request_id: u32) -> Result<u32, Error> {
    check_11bit(request_id, "request_id")?;
    let filter_id = request_id + GMLAN_FILTER_OFFSET;
    check_11bit(filter_id, "filter_id")?;
    Ok(filter_id)
}

/// Build the request/response pair for a GM GMLAN request ID.
///
/// # Errors
/// [`Error::CanIdRange`] if the request ID or the resulting response ID is above 0x7FF, or
/// [`Error::ReservedCanId`] if either is in an ISO-reserved range (Annex B).
pub fn gmlan_pair(request_id: u32) -> Result<NormalAddressing, Error> {
    NormalAddressing::new(request_id, gmlan_response_id(request_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_11bit_id_is_rejected() {
        assert!(NormalAddressing::new(0x7E0, 0x7E8).is_ok());
        assert_eq!(
            NormalAddressing::new(0x800, 0),
            Err(Error::CanIdRange {
                label: "request_id",
                value: 0x800
            })
        );
    }

    #[test]
    fn gmlan_response_offset() {
        assert_eq!(gmlan_response_id(0x241), Ok(0x641));
        // A request whose +0x400 response would exceed 11 bits is rejected.
        assert!(gmlan_response_id(0x400).is_err());
    }

    #[test]
    fn gmlan_filter_offset() {
        assert_eq!(gmlan_filter_id(0x241), Ok(0x541));
        let pair = gmlan_pair(0x241).unwrap();
        assert_eq!((pair.request_id(), pair.response_id()), (0x241, 0x641));
    }

    #[test]
    fn reserved_can_ids_are_rejected() {
        // The two ISO-reserved ranges (Annex B, Table B.1).
        for id in [0x7F4, 0x7F5, 0x7F6, 0x7FA, 0x7FB] {
            assert!(is_reserved_can_id(id), "{id:#x}");
            assert_eq!(
                NormalAddressing::new(id, 0x7E8),
                Err(Error::ReservedCanId {
                    label: "request_id",
                    value: id
                })
            );
        }
        // The identifiers just outside each range are usable.
        for id in [0x7F3, 0x7F7, 0x7F9, 0x7FC] {
            assert!(!is_reserved_can_id(id), "{id:#x}");
            assert!(NormalAddressing::new(id, 0x7E8).is_ok());
        }
        // A GMLAN request whose +0x400 response lands in a reserved range is rejected.
        assert_eq!(
            gmlan_response_id(0x3FA), // 0x3FA + 0x400 = 0x7FA, reserved
            Err(Error::ReservedCanId {
                label: "response_id",
                value: 0x7FA
            })
        );
    }

    #[test]
    fn normal_fixed_and_mixed_29bit_ids() {
        // Tester (N_TA 0xF1) to ECU (N_SA 0x00): the well-known 0x18DA F1 00 physical request.
        assert_eq!(normal_fixed_id(0xF1, 0x00, false), 0x18DA_F100);
        assert_eq!(normal_fixed_id(0x33, 0xF1, true), 0x18DB_33F1);
        assert_eq!(mixed_id_29bit(0xF1, 0x00, false), 0x18CE_F100);
        assert_eq!(mixed_id_29bit(0x33, 0xF1, true), 0x18CD_33F1);
        assert!(normal_fixed_id(0xFF, 0xFF, true) <= CAN_29BIT_MAX);
    }
}
