//! 11-bit normal addressing, and the GM GMLAN request/response/filter offset convention.
//!
//! Normal addressing (ISO 15765-2:2016, §10.3.2): the whole `N_AI` is the CAN identifier, so the
//! CAN data field is N_PCI + N_Data with no in-band address byte — which is what the [`crate::pci`]
//! module encodes. Extended and mixed addressing (§10.3.4, §10.3.5) place an address byte in the
//! first data byte; they are a planned row.
//!
//! The GMLAN 11-bit offsets are documented, read-only, in the `volt` research notes
//! (`docs/OBD-CONFIG-DID-APPENDIX.md`): a GM physical response ID is the request ID + 0x400, and
//! the flow/acceptance filter ID is the request ID + 0x300.

use crate::error::Error;

/// The largest 11-bit CAN identifier (0x7FF).
pub const CAN_11BIT_MAX: u32 = 0x7FF;
/// The GM GMLAN physical response offset (request + 0x400).
pub const GMLAN_RESPONSE_OFFSET: u32 = 0x400;
/// The GM GMLAN flow/acceptance filter offset (request + 0x300).
pub const GMLAN_FILTER_OFFSET: u32 = 0x300;

fn check_11bit(value: u32, label: &'static str) -> Result<(), Error> {
    if value > CAN_11BIT_MAX {
        return Err(Error::CanIdRange { label, value });
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
    /// Build a request/response pair, rejecting an identifier wider than 11 bits.
    ///
    /// # Errors
    /// [`Error::CanIdRange`] if either identifier is above 0x7FF.
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
/// [`Error::CanIdRange`] if the request ID, or the resulting response ID, is above 0x7FF.
pub fn gmlan_response_id(request_id: u32) -> Result<u32, Error> {
    check_11bit(request_id, "request_id")?;
    let response_id = request_id + GMLAN_RESPONSE_OFFSET;
    check_11bit(response_id, "response_id")?;
    Ok(response_id)
}

/// The GM GMLAN flow/acceptance filter ID for a request ID (request + 0x300).
///
/// # Errors
/// [`Error::CanIdRange`] if the request ID, or the resulting filter ID, is above 0x7FF.
pub fn gmlan_filter_id(request_id: u32) -> Result<u32, Error> {
    check_11bit(request_id, "request_id")?;
    let filter_id = request_id + GMLAN_FILTER_OFFSET;
    check_11bit(filter_id, "filter_id")?;
    Ok(filter_id)
}

/// Build the request/response pair for a GM GMLAN request ID.
///
/// # Errors
/// [`Error::CanIdRange`] if the request ID, or the resulting response ID, is above 0x7FF.
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
}
