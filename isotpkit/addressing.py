"""11-bit normal addressing, and the GM GMLAN request/response/filter offset convention.

Normal addressing (ISO 15765-2:2016, 10.3.2): the whole N_AI is the CAN identifier, so the
CAN data field is N_PCI + N_Data with no in-band address byte — which is what the ``pci``
module encodes. Extended and mixed addressing (10.3.4, 10.3.5) place an address byte in the
first data byte; they are a ROADMAP item.

The GMLAN 11-bit offsets are documented, read-only, in the ``volt`` research notes
(``docs/OBD-CONFIG-DID-APPENDIX.md``): a GM physical response ID is the request ID + 0x400,
and the flow/acceptance filter ID is the request ID + 0x300.
"""

from __future__ import annotations

from dataclasses import dataclass

CAN_11BIT_MAX = 0x7FF
GMLAN_RESPONSE_OFFSET = 0x400
GMLAN_FILTER_OFFSET = 0x300


def _check_11bit(can_id: int, label: str) -> None:
    if not 0 <= can_id <= CAN_11BIT_MAX:
        raise ValueError(f"{label} {can_id:#x} is outside the 11-bit range 0..{CAN_11BIT_MAX:#x}")


@dataclass(frozen=True, slots=True)
class NormalAddressing:
    """A normal-addressing 11-bit request/response CAN identifier pair."""

    request_id: int
    response_id: int

    def __post_init__(self) -> None:
        _check_11bit(self.request_id, "request_id")
        _check_11bit(self.response_id, "response_id")


def gmlan_response_id(request_id: int) -> int:
    """The GM GMLAN physical response ID for a request ID (request + 0x400)."""
    _check_11bit(request_id, "request_id")
    response_id = request_id + GMLAN_RESPONSE_OFFSET
    _check_11bit(response_id, "response_id")
    return response_id


def gmlan_filter_id(request_id: int) -> int:
    """The GM GMLAN flow/acceptance filter ID for a request ID (request + 0x300)."""
    _check_11bit(request_id, "request_id")
    filter_id = request_id + GMLAN_FILTER_OFFSET
    _check_11bit(filter_id, "filter_id")
    return filter_id


def gmlan_pair(request_id: int) -> NormalAddressing:
    """Build the request/response pair for a GM GMLAN request ID."""
    return NormalAddressing(request_id, gmlan_response_id(request_id))
