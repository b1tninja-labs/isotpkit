"""The network-layer timing parameters are named from the standard (9.8.1, Table 21)."""

from __future__ import annotations

from isotpkit import timing


def test_timing_parameters_are_named():
    assert timing.parameter_names() == ("N_As", "N_Ar", "N_Bs", "N_Br", "N_Cs", "N_Cr")


def test_timeouts_match_table_21():
    assert timing.N_AS.timeout_ms == 1000
    assert timing.N_AR.timeout_ms == 1000
    assert timing.N_BS.timeout_ms == 1000
    assert timing.N_CR.timeout_ms == 1000
    # N_Br and N_Cs carry a performance requirement, not a timeout.
    assert timing.N_BR.timeout_ms is None
    assert timing.N_CS.timeout_ms is None


def test_timeout_tolerance_is_fifty_percent():
    assert timing.TIMEOUT_TOLERANCE == 0.5
