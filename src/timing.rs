//! Network-layer timing parameters, named from the standard — no live timer runs here.
//!
//! Grounded in ISO 15765-2:2016, §9.7 (`N_WFTmax`) and §9.8.1 (Timing parameters, Table 21).
//! There is no bus in this crate, so these are named constants and their standard timeout values
//! for documentation and for any sender/receiver built on top, not a timer loop.

/// One network-layer timing parameter and its standard timeout.
///
/// `timeout_ms` is the Table 21 timeout where the standard fixes one, and `None` where Table 21
/// states a performance requirement instead of a timeout (`N_Br`, `N_Cs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingParameter {
    name: &'static str,
    description: &'static str,
    timeout_ms: Option<u32>,
}

impl TimingParameter {
    /// The parameter's name (for example `N_As`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// A one-line description of what the parameter times.
    #[must_use]
    pub const fn description(self) -> &'static str {
        self.description
    }

    /// The Table 21 timeout in milliseconds, or `None` for a performance requirement.
    #[must_use]
    pub const fn timeout_ms(self) -> Option<u32> {
        self.timeout_ms
    }
}

/// `N_As`: transmission of any N_PDU on the sender side.
pub const N_AS: TimingParameter = TimingParameter {
    name: "N_As",
    description: "Transmission of any N_PDU on the sender side",
    timeout_ms: Some(1000),
};
/// `N_Ar`: transmission of any N_PDU on the receiver side.
pub const N_AR: TimingParameter = TimingParameter {
    name: "N_Ar",
    description: "Transmission of any N_PDU on the receiver side",
    timeout_ms: Some(1000),
};
/// `N_Bs`: time until reception of the next `FlowControl` N_PDU.
pub const N_BS: TimingParameter = TimingParameter {
    name: "N_Bs",
    description: "Time until reception of the next FlowControl N_PDU",
    timeout_ms: Some(1000),
};
/// `N_Br`: time until transmission of the next `FlowControl` N_PDU (a performance requirement).
pub const N_BR: TimingParameter = TimingParameter {
    name: "N_Br",
    description: "Time until transmission of the next FlowControl N_PDU",
    timeout_ms: None,
};
/// `N_Cs`: time until transmission of the next `ConsecutiveFrame` N_PDU (a performance requirement).
pub const N_CS: TimingParameter = TimingParameter {
    name: "N_Cs",
    description: "Time until transmission of the next ConsecutiveFrame N_PDU",
    timeout_ms: None,
};
/// `N_Cr`: time until reception of the next `ConsecutiveFrame` N_PDU.
pub const N_CR: TimingParameter = TimingParameter {
    name: "N_Cr",
    description: "Time until reception of the next ConsecutiveFrame N_PDU",
    timeout_ms: Some(1000),
};

/// The six network-layer timing parameters, in Table 21 order.
pub const PARAMETERS: [TimingParameter; 6] = [N_AS, N_AR, N_BS, N_BR, N_CS, N_CR];

/// "The real timeout shall occur no later than at the specified timeout value + 50 %." (§9.8.1)
pub const TIMEOUT_TOLERANCE: f64 = 0.5;

// `N_WFTmax` (§9.7): the maximum number of FC.WAIT frames a receiver may send in a row. It is
// local to the peer, not carried on the wire, and user-defined at system generation (no fixed
// default); 0 means the receiver uses FC.CTS only and never FC.WAIT.

/// The names of the six network-layer timing parameters, in Table 21 order.
#[must_use]
pub fn parameter_names() -> [&'static str; 6] {
    [
        N_AS.name(),
        N_AR.name(),
        N_BS.name(),
        N_BR.name(),
        N_CS.name(),
        N_CR.name(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_parameters_are_named() {
        assert_eq!(
            parameter_names(),
            ["N_As", "N_Ar", "N_Bs", "N_Br", "N_Cs", "N_Cr"]
        );
    }

    #[test]
    fn timeouts_follow_table_21() {
        assert_eq!(N_AS.timeout_ms(), Some(1000));
        assert_eq!(N_CR.timeout_ms(), Some(1000));
        assert_eq!(N_BR.timeout_ms(), None);
        assert_eq!(N_CS.timeout_ms(), None);
    }
}
