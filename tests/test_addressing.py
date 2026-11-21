"""11-bit normal addressing and the GM GMLAN offset convention (10.3.2 + volt notes)."""

from __future__ import annotations

import pytest

from isotpkit.addressing import (
    NormalAddressing,
    gmlan_filter_id,
    gmlan_pair,
    gmlan_response_id,
)


def test_gmlan_response_offset():
    # Request 0x24X -> response 0x64X (request + 0x400).
    assert gmlan_response_id(0x241) == 0x641
    assert gmlan_response_id(0x24F) == 0x64F


def test_gmlan_filter_offset():
    # Request 0x24X -> filter 0x54X (request + 0x300).
    assert gmlan_filter_id(0x241) == 0x541


def test_gmlan_pair_builds_request_and_response():
    pair = gmlan_pair(0x241)
    assert pair.request_id == 0x241
    assert pair.response_id == 0x641


def test_non_11bit_id_is_rejected():
    with pytest.raises(ValueError, match="11-bit"):
        NormalAddressing(0x800, 0x700)
    with pytest.raises(ValueError, match="11-bit"):
        gmlan_response_id(0x500)  # 0x500 + 0x400 = 0x900, beyond 0x7FF
