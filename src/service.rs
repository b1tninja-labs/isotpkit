//! The network-layer service primitives and their service data unit parameters (§8.2, §8.3).
//!
//! §8 specifies the interface the network layer offers the layer above it: the `N_USData` and
//! `N_ChangeParameter` service primitives, built from the service data unit (SDU) parameters of
//! §8.3 — `Mtype`, the address information `N_AI` (`N_SA`/`N_TA`/`N_TAtype`/`N_AE`), `<Length>`,
//! `<MessageData>`, `<Parameter>`/`<Parameter_Value>`, `<N_Result>` and `<Result_ChangeParameter>`.
//!
//! This is still sans-I/O: the module is the *typed interface* — the primitives and their
//! parameters as data — not a running service. It owns no bus and no timer; the [`session`] module
//! paces the transfer and the caller turns an outcome into the matching [`Primitive`]. The outcome
//! parameter is the [`NResult`] model of §8.3.7, which already maps from the crate's
//! [`Error`].
//!
//! [`session`]: crate::session
//!
//! # Example
//!
//! ```
//! use isotpkit::NResult;
//! use isotpkit::service::{AddressInfo, Primitive, TargetAddressType};
//!
//! // A diagnostics request (no N_AE) to physically addressed ECU 0x10 from tester 0xF1.
//! let ai = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
//! let request = Primitive::us_data_request(ai, b"\x22\xf1\x90".to_vec()).unwrap();
//! assert_eq!(request.address_info().n_ta(), 0x10);
//!
//! // The network layer later confirms the request's completion with an <N_Result>.
//! let confirm = Primitive::us_data_confirm(ai, NResult::Ok);
//! assert!(matches!(confirm, Primitive::USDataConfirm { .. }));
//! ```

use alloc::vec::Vec;

use crate::error::Error;
use crate::result::NResult;

/// `Mtype`, the message type that selects which address-information parameters apply (§8.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mtype {
    /// `diagnostics`: `N_AI` is `N_SA`, `N_TA`, `N_TAtype` (no `N_AE`).
    Diagnostics,
    /// `remote diagnostics`: `N_AI` additionally carries the address extension `N_AE`.
    RemoteDiagnostics,
}

impl Mtype {
    /// Whether this message type carries the address extension `N_AE` (§8.3.1).
    #[must_use]
    pub const fn has_address_extension(self) -> bool {
        matches!(self, Mtype::RemoteDiagnostics)
    }

    /// The canonical name of the message type.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Mtype::Diagnostics => "diagnostics",
            Mtype::RemoteDiagnostics => "remote diagnostics",
        }
    }
}

/// `N_TAtype`, the communication model of the target address (§8.3.2.4, Table 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetAddressType {
    /// Physical: 1-to-1 communication, supported for every network-layer message.
    Physical,
    /// Functional: 1-to-n communication, supported for `SingleFrame` transmission only (Table 4b).
    Functional,
}

impl TargetAddressType {
    /// Whether a segmented (multi-frame) message may use this model. Functional addressing is
    /// `SingleFrame`-only (§8.3.2.4, Table 4, note b), so only physical addressing may segment.
    #[must_use]
    pub const fn supports_segmentation(self) -> bool {
        matches!(self, TargetAddressType::Physical)
    }

    /// The canonical name of the address type.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TargetAddressType::Physical => "physical",
            TargetAddressType::Functional => "functional",
        }
    }
}

/// `N_AI`, the address information of a service call (§8.3.2).
///
/// It names the source (`N_SA`), the target (`N_TA`) and its communication model (`N_TAtype`), and
/// — for remote diagnostics only — the address extension (`N_AE`). The constructors tie `N_AE`'s
/// presence to the `Mtype`, exactly as §8.3.1 requires: [`diagnostics`](AddressInfo::diagnostics)
/// has no `N_AE`, [`remote_diagnostics`](AddressInfo::remote_diagnostics) always does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressInfo {
    mtype: Mtype,
    n_sa: u8,
    n_ta: u8,
    n_ta_type: TargetAddressType,
    n_ae: Option<u8>,
}

#[allow(
    clippy::similar_names,
    reason = "n_sa, n_ta and n_ae are the ISO 15765-2 parameter names (§8.3.2)"
)]
impl AddressInfo {
    /// Diagnostics address information (`Mtype` = diagnostics): no address extension.
    #[must_use]
    pub const fn diagnostics(n_sa: u8, n_ta: u8, n_ta_type: TargetAddressType) -> AddressInfo {
        AddressInfo {
            mtype: Mtype::Diagnostics,
            n_sa,
            n_ta,
            n_ta_type,
            n_ae: None,
        }
    }

    /// Remote-diagnostics address information (`Mtype` = remote diagnostics): carries `N_AE`.
    #[must_use]
    pub const fn remote_diagnostics(
        n_sa: u8,
        n_ta: u8,
        n_ta_type: TargetAddressType,
        n_ae: u8,
    ) -> AddressInfo {
        AddressInfo {
            mtype: Mtype::RemoteDiagnostics,
            n_sa,
            n_ta,
            n_ta_type,
            n_ae: Some(n_ae),
        }
    }

    /// The message type (`Mtype`).
    #[must_use]
    pub const fn mtype(&self) -> Mtype {
        self.mtype
    }

    /// The network source address (`N_SA`).
    #[must_use]
    pub const fn n_sa(&self) -> u8 {
        self.n_sa
    }

    /// The network target address (`N_TA`).
    #[must_use]
    pub const fn n_ta(&self) -> u8 {
        self.n_ta
    }

    /// The target address type (`N_TAtype`).
    #[must_use]
    pub const fn n_ta_type(&self) -> TargetAddressType {
        self.n_ta_type
    }

    /// The address extension (`N_AE`), present only for remote diagnostics (§8.3.2.5).
    #[must_use]
    pub const fn n_ae(&self) -> Option<u8> {
        self.n_ae
    }
}

/// `<Parameter>`, an internal network-layer parameter a `N_ChangeParameter` request can set (§8.3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parameter {
    /// `BS`, the `BlockSize` the receiver advertises.
    BlockSize,
    /// `STmin`, the minimum separation time the receiver advertises.
    SeparationTimeMin,
}

impl Parameter {
    /// The canonical name of the parameter.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Parameter::BlockSize => "BS",
            Parameter::SeparationTimeMin => "STmin",
        }
    }
}

/// `<Result_ChangeParameter>`, the outcome of a `N_ChangeParameter` request (§8.3.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResultChangeParameter {
    /// `N_OK`: the parameter change completed successfully (both sides).
    Ok,
    /// `N_RX_ON`: refused because a message was being received (receiver only).
    RxOn,
    /// `N_WRONG_PARAMETER`: the `<Parameter>` is undefined (both sides).
    WrongParameter,
    /// `N_WRONG_VALUE`: the `<Parameter_Value>` is out of range (both sides).
    WrongValue,
}

impl ResultChangeParameter {
    /// The canonical `N_…` name of the result.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            ResultChangeParameter::Ok => "N_OK",
            ResultChangeParameter::RxOn => "N_RX_ON",
            ResultChangeParameter::WrongParameter => "N_WRONG_PARAMETER",
            ResultChangeParameter::WrongValue => "N_WRONG_VALUE",
        }
    }
}

/// Decide the outcome of a `N_ChangeParameter.request` (§8.2.5, §8.3.8).
///
/// A parameter change "is always possible, except after reception of the FF
/// (`N_USData_FF.indication`) and until the end of reception" (§8.2.5): while a segmented reception
/// is in progress the change is refused with `N_RX_ON`, otherwise it succeeds (`N_OK`). The typed
/// [`Parameter`]/[`Parameter_Value`](u8) API cannot express an undefined parameter or an
/// out-of-range 8-bit value, so `N_WRONG_PARAMETER`/`N_WRONG_VALUE` are reachable only from a raw
/// request and are not produced here.
#[must_use]
pub const fn change_parameter_result(reception_in_progress: bool) -> ResultChangeParameter {
    if reception_in_progress {
        ResultChangeParameter::RxOn
    } else {
        ResultChangeParameter::Ok
    }
}

/// One network-layer service primitive (§8.2), each carrying its [`AddressInfo`] and SDU parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Primitive {
    /// `N_USData.request`: ask to transmit `<MessageData>` of `<Length>` bytes (§8.2.1).
    USDataRequest {
        /// The address information of the transfer.
        ai: AddressInfo,
        /// The message bytes to send (`<MessageData>`; `<Length>` is its length).
        data: Vec<u8>,
    },
    /// `N_USData.confirm`: signal the completion or failure of a request (§8.2.2).
    USDataConfirm {
        /// The address information of the confirmed request.
        ai: AddressInfo,
        /// The outcome (`<N_Result>`).
        result: NResult,
    },
    /// `N_USData_FF.indication`: a `FirstFrame` of a segmented message has arrived (§8.2.3).
    USDataFfIndication {
        /// The address information of the incoming message.
        ai: AddressInfo,
        /// The announced total message length (`<Length>`, from `FF_DL`).
        length: u32,
    },
    /// `N_USData.indication`: deliver a received message, or indicate a failed reception (§8.2.4).
    USDataIndication {
        /// The address information of the received message.
        ai: AddressInfo,
        /// The outcome (`<N_Result>`); `data` is meaningful only when this is `N_OK`.
        result: NResult,
        /// The received bytes (`<MessageData>`), valid only when `result` is `N_OK`.
        data: Vec<u8>,
    },
    /// `N_ChangeParameter.request`: set an internal parameter's value (§8.2.5).
    ChangeParameterRequest {
        /// The address information the change applies to.
        ai: AddressInfo,
        /// Which parameter to change (`<Parameter>`).
        parameter: Parameter,
        /// The new value (`<Parameter_Value>`, 8 bits).
        value: u8,
    },
    /// `N_ChangeParameter.confirm`: report the outcome of a parameter change (§8.2.6).
    ChangeParameterConfirm {
        /// The address information the change applied to.
        ai: AddressInfo,
        /// Which parameter was changed (`<Parameter>`).
        parameter: Parameter,
        /// The outcome (`<Result_ChangeParameter>`).
        result: ResultChangeParameter,
    },
}

impl Primitive {
    /// Build an `N_USData.request` (§8.2.1).
    ///
    /// # Errors
    /// [`Error::EmptyMessage`] when `data` is empty and [`Error::MessageTooLong`] when it exceeds the
    /// 32-bit `<Length>` maximum (§8.3.3, range `0x0000_0001`..=`0xFFFF_FFFF`).
    pub fn us_data_request(ai: AddressInfo, data: Vec<u8>) -> Result<Primitive, Error> {
        if data.is_empty() {
            return Err(Error::EmptyMessage);
        }
        if u32::try_from(data.len()).is_err() {
            return Err(Error::MessageTooLong(data.len()));
        }
        Ok(Primitive::USDataRequest { ai, data })
    }

    /// Build an `N_USData.confirm` carrying the outcome `<N_Result>` (§8.2.2).
    #[must_use]
    pub const fn us_data_confirm(ai: AddressInfo, result: NResult) -> Primitive {
        Primitive::USDataConfirm { ai, result }
    }

    /// Build an `N_USData_FF.indication` for an arriving segmented message (§8.2.3).
    ///
    /// # Errors
    /// [`Error::EmptyMessage`] when `length` is 0; `<Length>` is at least 1 (§8.3.3).
    pub const fn us_data_ff_indication(ai: AddressInfo, length: u32) -> Result<Primitive, Error> {
        if length == 0 {
            return Err(Error::EmptyMessage);
        }
        Ok(Primitive::USDataFfIndication { ai, length })
    }

    /// Build an `N_USData.indication` from a reception outcome (§8.2.4).
    ///
    /// `Ok(data)` delivers the message with `<N_Result>` = `N_OK`; `Err(result)` indicates a failed
    /// reception and carries no data, enforcing the rule that `<MessageData>`/`<Length>` are valid
    /// only when `<N_Result>` is `N_OK`.
    #[must_use]
    pub fn us_data_indication(ai: AddressInfo, outcome: Result<Vec<u8>, NResult>) -> Primitive {
        match outcome {
            Ok(data) => Primitive::USDataIndication {
                ai,
                result: NResult::Ok,
                data,
            },
            Err(result) => Primitive::USDataIndication {
                ai,
                result,
                data: Vec::new(),
            },
        }
    }

    /// Build an `N_ChangeParameter.request` (§8.2.5).
    #[must_use]
    pub const fn change_parameter_request(
        ai: AddressInfo,
        parameter: Parameter,
        value: u8,
    ) -> Primitive {
        Primitive::ChangeParameterRequest {
            ai,
            parameter,
            value,
        }
    }

    /// Build an `N_ChangeParameter.confirm` (§8.2.6).
    #[must_use]
    pub const fn change_parameter_confirm(
        ai: AddressInfo,
        parameter: Parameter,
        result: ResultChangeParameter,
    ) -> Primitive {
        Primitive::ChangeParameterConfirm {
            ai,
            parameter,
            result,
        }
    }

    /// The address information (`N_AI`) carried by this primitive.
    #[must_use]
    pub const fn address_info(&self) -> &AddressInfo {
        match self {
            Primitive::USDataRequest { ai, .. }
            | Primitive::USDataConfirm { ai, .. }
            | Primitive::USDataFfIndication { ai, .. }
            | Primitive::USDataIndication { ai, .. }
            | Primitive::ChangeParameterRequest { ai, .. }
            | Primitive::ChangeParameterConfirm { ai, .. } => ai,
        }
    }

    /// The received `<MessageData>` of an `N_USData.indication`, valid only when its `<N_Result>` is
    /// `N_OK` (§8.2.4); `None` for any other primitive or a failed indication.
    #[must_use]
    pub fn message_data(&self) -> Option<&[u8]> {
        match self {
            Primitive::USDataIndication {
                result: NResult::Ok,
                data,
                ..
            } => Some(data),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn mtype_governs_the_address_extension() {
        let diag = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
        assert_eq!(diag.mtype(), Mtype::Diagnostics);
        assert!(!diag.mtype().has_address_extension());
        assert_eq!(diag.n_ae(), None);

        let remote =
            AddressInfo::remote_diagnostics(0xF1, 0x10, TargetAddressType::Functional, 0x99);
        assert_eq!(remote.mtype(), Mtype::RemoteDiagnostics);
        assert!(remote.mtype().has_address_extension());
        assert_eq!(remote.n_ae(), Some(0x99));
    }

    #[test]
    fn functional_addressing_is_single_frame_only() {
        assert!(TargetAddressType::Physical.supports_segmentation());
        assert!(!TargetAddressType::Functional.supports_segmentation());
    }

    #[test]
    fn request_validates_the_length_parameter() {
        let ai = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
        assert!(Primitive::us_data_request(ai, vec![0x3E, 0x00]).is_ok());
        assert_eq!(
            Primitive::us_data_request(ai, Vec::new()),
            Err(Error::EmptyMessage)
        );
    }

    #[test]
    fn indication_exposes_data_only_when_ok() {
        let ai = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
        let delivered = Primitive::us_data_indication(ai, Ok(vec![0x62, 0xf1, 0x90]));
        assert_eq!(delivered.message_data(), Some(&[0x62, 0xf1, 0x90][..]));

        // A failed reception carries the result but no message data (§8.2.4).
        let failed = Primitive::us_data_indication(ai, Err(NResult::WrongSn));
        assert_eq!(failed.message_data(), None);
        match failed {
            Primitive::USDataIndication { result, .. } => assert_eq!(result, NResult::WrongSn),
            other => panic!("expected an indication, got {other:?}"),
        }
    }

    #[test]
    fn confirm_reports_the_n_result() {
        let ai = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
        // The caller maps a session outcome onto the confirm's <N_Result>.
        let outcome: Result<(), Error> = Err(Error::BufferOverflow);
        let result = outcome.map_or_else(|e| NResult::from(&e), |()| NResult::Ok);
        assert_eq!(
            Primitive::us_data_confirm(ai, result),
            Primitive::USDataConfirm {
                ai,
                result: NResult::BufferOvflw,
            }
        );
    }

    #[test]
    fn change_parameter_is_refused_during_reception() {
        // §8.2.5: a change is possible except from the FF until the end of reception.
        assert_eq!(change_parameter_result(false), ResultChangeParameter::Ok);
        assert_eq!(change_parameter_result(true), ResultChangeParameter::RxOn);

        let ai = AddressInfo::diagnostics(0xF1, 0x10, TargetAddressType::Physical);
        let confirm = Primitive::change_parameter_confirm(
            ai,
            Parameter::BlockSize,
            change_parameter_result(true),
        );
        assert_eq!(
            confirm,
            Primitive::ChangeParameterConfirm {
                ai,
                parameter: Parameter::BlockSize,
                result: ResultChangeParameter::RxOn,
            }
        );
    }

    #[test]
    fn address_info_is_reachable_from_every_primitive() {
        let ai = AddressInfo::diagnostics(0x01, 0x02, TargetAddressType::Physical);
        let primitives = [
            Primitive::us_data_request(ai, vec![1]).unwrap(),
            Primitive::us_data_confirm(ai, NResult::Ok),
            Primitive::us_data_ff_indication(ai, 100).unwrap(),
            Primitive::us_data_indication(ai, Ok(vec![1, 2])),
            Primitive::change_parameter_request(ai, Parameter::SeparationTimeMin, 0x0A),
            Primitive::change_parameter_confirm(
                ai,
                Parameter::BlockSize,
                ResultChangeParameter::Ok,
            ),
        ];
        for p in &primitives {
            assert_eq!(p.address_info().n_ta(), 0x02);
        }
    }
}
