"""FlowControl parameters: FlowStatus, BlockSize and the STmin encoding.

Grounded in ISO 15765-2:2016, 9.6.5 (FlowControl N_PCI parameter definition).
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum


class FlowStatus(IntEnum):
    """FlowStatus (FS), the low nibble of the FC N_PCI byte (ISO 15765-2:2016, 9.6.5.1, Table 18)."""

    CONTINUE_TO_SEND = 0x0  # CTS: sender may send up to BS ConsecutiveFrames
    WAIT = 0x1  # WAIT: sender keeps waiting for a new FC and restarts N_Bs; BS/STmin ignored
    OVERFLOW = 0x2  # OVFLW: receiver buffer too small for FF_DL; sender aborts (N_BUFFER_OVFLW)


# A reserved STmin value is treated as the longest value, 0x7F = 127 ms (9.6.5.5).
RESERVED_STMIN_FALLBACK_US = 127_000

_STMIN_MS_MAX = 0x7F  # 0x00..0x7F are 0..127 ms
_STMIN_US_LOW = 0xF1  # 0xF1..0xF9 are 100..900 us
_STMIN_US_HIGH = 0xF9


@dataclass(frozen=True, slots=True)
class SeparationTime:
    """A parsed STmin byte (ISO 15765-2:2016, 9.6.5.4, Table 20).

    ``raw`` is the on-wire byte, ``microseconds`` the resolved minimum separation time,
    and ``reserved`` whether ``raw`` fell in a reserved range (then ``microseconds`` is
    the 127 ms fallback the sender must use per 9.6.5.5).
    """

    raw: int
    microseconds: int
    reserved: bool

    @classmethod
    def from_byte(cls, raw: int) -> SeparationTime:
        """Parse one STmin byte into its minimum-separation time in microseconds."""
        if not 0x00 <= raw <= 0xFF:
            raise ValueError(f"STmin byte out of range: {raw:#x}")
        if raw <= _STMIN_MS_MAX:
            return cls(raw, raw * 1000, reserved=False)
        if _STMIN_US_LOW <= raw <= _STMIN_US_HIGH:
            return cls(raw, (raw - 0xF0) * 100, reserved=False)
        # 0x80..0xF0 and 0xFA..0xFF are reserved.
        return cls(raw, RESERVED_STMIN_FALLBACK_US, reserved=True)

    def to_byte(self) -> int:
        """The on-wire STmin byte."""
        return self.raw


def encode_stmin(microseconds: int) -> int:
    """Encode a minimum separation time in microseconds into an STmin byte.

    Whole milliseconds 0..127 use the 0x00..0x7F range; 100..900 us in 100 us steps use
    the 0xF1..0xF9 range. Any other value is not representable and raises ``ValueError``.
    """
    if microseconds < 0:
        raise ValueError(f"STmin cannot be negative: {microseconds}")
    if microseconds % 1000 == 0 and microseconds // 1000 <= 127:
        return microseconds // 1000
    if 100 <= microseconds <= 900 and microseconds % 100 == 0:
        return 0xF0 + microseconds // 100
    raise ValueError(f"STmin {microseconds} us is not representable in ISO 15765-2 STmin (9.6.5.4)")
