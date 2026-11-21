"""FlowStatus, BlockSize and the STmin encoding (ISO 15765-2:2016, 9.6.5)."""

from __future__ import annotations

import pytest

from isotpkit.flowcontrol import (
    RESERVED_STMIN_FALLBACK_US,
    FlowStatus,
    SeparationTime,
    encode_stmin,
)


def test_flow_status_values():
    assert FlowStatus.CONTINUE_TO_SEND == 0x0
    assert FlowStatus.WAIT == 0x1
    assert FlowStatus.OVERFLOW == 0x2


def test_stmin_milliseconds():
    assert SeparationTime.from_byte(0x00).microseconds == 0
    assert SeparationTime.from_byte(0x0A).microseconds == 10_000
    assert SeparationTime.from_byte(0x7F).microseconds == 127_000
    assert SeparationTime.from_byte(0x0A).reserved is False


def test_stmin_microseconds():
    assert SeparationTime.from_byte(0xF1).microseconds == 100
    assert SeparationTime.from_byte(0xF5).microseconds == 500
    assert SeparationTime.from_byte(0xF9).microseconds == 900


def test_reserved_stmin_falls_back():
    for raw in (0x80, 0xF0, 0xFA, 0xFF):
        parsed = SeparationTime.from_byte(raw)
        assert parsed.reserved is True
        assert parsed.microseconds == RESERVED_STMIN_FALLBACK_US


def test_encode_stmin_roundtrip():
    for raw in (0x00, 0x0A, 0x7F, 0xF1, 0xF5, 0xF9):
        assert encode_stmin(SeparationTime.from_byte(raw).microseconds) == raw


def test_encode_stmin_rejects_unrepresentable():
    with pytest.raises(ValueError, match="not representable"):
        encode_stmin(1_500)  # 1.5 ms is neither a whole ms nor a 100 us step
    with pytest.raises(ValueError, match="not representable"):
        encode_stmin(128_000)  # beyond 127 ms
