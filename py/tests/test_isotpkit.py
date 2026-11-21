"""Smoke tests for the isotpkit Python bindings.

Build the extension first (``maturin develop`` from ``py/``, or build the cdylib and place it as
``isotpkit.pyd`` / ``isotpkit.so`` on the path), then run ``pytest``.
"""

import isotpkit


def test_single_frame_round_trips():
    message = bytes(range(3))
    frames = isotpkit.segment(message)
    assert len(frames) == 1
    assert isotpkit.reassemble(frames) == message


def test_multi_frame_round_trips():
    message = bytes((i * 7 + 3) & 0xFF for i in range(100))
    frames = isotpkit.segment(message)
    assert len(frames) > 1
    assert isotpkit.reassemble(frames) == message


def test_describe_single_frame():
    assert isotpkit.describe(bytes.fromhex("023e00")) == "SingleFrame SF_DL=2 data=3e00"


def test_stmin_round_trip():
    assert isotpkit.stmin_microseconds(0xF1) == 100
    assert isotpkit.stmin_microseconds(0x0A) == 10_000
    assert isotpkit.encode_stmin(100) == 0xF1
    assert isotpkit.encode_stmin(10_000) == 0x0A


def test_gmlan_offsets():
    assert isotpkit.gmlan_response_id(0x241) == 0x641
    assert isotpkit.gmlan_filter_id(0x241) == 0x541


def test_errors_raise_value_error():
    import pytest

    with pytest.raises(ValueError):
        isotpkit.segment(b"")
    with pytest.raises(ValueError):
        isotpkit.encode_stmin(50)
