"""The four ISO-TP N_PDU types and their classic-CAN (8-byte) frame codec.

Grounded in ISO 15765-2:2016, 9.6 (Protocol control information specification) and
9.4 (Transport layer protocol data units). This module handles classic CAN where the
data-link length CAN_DL is at most 8 bytes and normal addressing (no in-band address
byte). CAN FD (CAN_DL > 8), the SingleFrame escape (SF_DL > 7) and the 32-bit FirstFrame
escape (FF_DL > 4095) are ROADMAP items and are rejected here rather than guessed.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum

from isotpkit.flowcontrol import FlowStatus

CLASSIC_CAN_DL = 8  # classic CAN maximum data length
DEFAULT_PADDING = 0xCC  # default frame padding value (ISO 15765-2:2016, 10.4.2.1)
SF_DL_MAX = 7  # SingleFrame payload, normal addressing, CAN_DL = 8 => CAN_DL - 1
FF_DATA_LEN = 6  # FirstFrame payload bytes on classic CAN = CAN_DL - 2 PCI bytes
CF_DATA_MAX = 7  # ConsecutiveFrame payload bytes = CAN_DL - 1 PCI byte
FF_DL_MIN = 8  # minimum FF_DL, TX_DL = 8, normal addressing (9.6.3.1, Table 14)
FF_DL_12BIT_MAX = 0xFFF  # 4095, the largest FF_DL without the 32-bit escape
SN_MAX = 0xF  # SequenceNumber wraps 0x0..0xF (9.6.4.3)


class PciType(IntEnum):
    """N_PCItype, the high nibble of N_PCI byte #1 (ISO 15765-2:2016, 9.6.1, Table 8)."""

    SINGLE_FRAME = 0x0
    FIRST_FRAME = 0x1
    CONSECUTIVE_FRAME = 0x2
    FLOW_CONTROL = 0x3


def pci_type(byte1: int) -> PciType:
    """Classify N_PCI byte #1 by its high nibble; reject the reserved 0x4..0xF range."""
    nibble = byte1 >> 4
    try:
        return PciType(nibble)
    except ValueError:
        raise ValueError(f"reserved N_PCItype {nibble:#x} (ISO 15765-2:2016, 9.6.1, Table 8)") from None


def _require_frame(frame: bytes) -> None:
    if not frame:
        raise ValueError("empty CAN frame")
    if len(frame) > CLASSIC_CAN_DL:
        raise ValueError(
            f"classic CAN frame is at most {CLASSIC_CAN_DL} bytes, got {len(frame)}; CAN FD is a ROADMAP item"
        )


def _pad(frame: bytes, padding: int) -> bytes:
    if not 0x00 <= padding <= 0xFF:
        raise ValueError(f"padding byte out of range: {padding:#x}")
    return frame + bytes([padding]) * (CLASSIC_CAN_DL - len(frame))


@dataclass(frozen=True, slots=True)
class SingleFrame:
    """SF N_PDU: an unsegmented message that fits one frame (ISO 15765-2:2016, 9.6.2)."""

    data: bytes

    def __post_init__(self) -> None:
        if not 1 <= len(self.data) <= SF_DL_MAX:
            raise ValueError(f"SingleFrame payload must be 1..{SF_DL_MAX} bytes, got {len(self.data)}")

    @property
    def sf_dl(self) -> int:
        """SingleFrame data length (the low nibble of N_PCI byte #1)."""
        return len(self.data)

    def encode(self, *, pad: bool = True, padding: int = DEFAULT_PADDING) -> bytes:
        """The on-wire SF frame: ``0x0<SF_DL>`` then the data, padded to 8 when ``pad``."""
        frame = bytes([(PciType.SINGLE_FRAME << 4) | self.sf_dl]) + self.data
        return _pad(frame, padding) if pad else frame

    @classmethod
    def decode(cls, frame: bytes) -> SingleFrame:
        """Decode an SF frame, validating SF_DL against the available data bytes."""
        _require_frame(frame)
        if pci_type(frame[0]) is not PciType.SINGLE_FRAME:
            raise ValueError(f"not a SingleFrame: byte #1 is {frame[0]:#04x}")
        sf_dl = frame[0] & 0x0F
        if sf_dl == 0:
            raise ValueError("SF_DL 0 is the CAN FD SingleFrame escape (SF_DL > 7); a ROADMAP item")
        if sf_dl > len(frame) - 1:
            raise ValueError(f"SF_DL {sf_dl} exceeds the {len(frame) - 1} available data bytes")
        return cls(frame[1 : 1 + sf_dl])


@dataclass(frozen=True, slots=True)
class FirstFrame:
    """FF N_PDU: the first frame of a segmented message (ISO 15765-2:2016, 9.6.3).

    ``length`` is FF_DL, the full message length (12-bit, 8..4095 here). ``data`` is the
    first 6 payload bytes carried in the FF itself.
    """

    length: int
    data: bytes

    def __post_init__(self) -> None:
        if not FF_DL_MIN <= self.length <= FF_DL_12BIT_MAX:
            raise ValueError(f"FF_DL must be {FF_DL_MIN}..{FF_DL_12BIT_MAX}, got {self.length}")
        if len(self.data) != FF_DATA_LEN:
            raise ValueError(f"FirstFrame carries {FF_DATA_LEN} payload bytes, got {len(self.data)}")

    def encode(self) -> bytes:
        """The on-wire FF frame: ``0x1<FF_DL hi>`` ``<FF_DL lo>`` then 6 data bytes."""
        byte1 = (PciType.FIRST_FRAME << 4) | ((self.length >> 8) & 0x0F)
        return bytes([byte1, self.length & 0xFF]) + self.data

    @classmethod
    def decode(cls, frame: bytes) -> FirstFrame:
        """Decode an FF frame, rejecting the 32-bit escape and CAN_DL < 8."""
        _require_frame(frame)
        if len(frame) < CLASSIC_CAN_DL:
            raise ValueError(f"FirstFrame requires CAN_DL >= {CLASSIC_CAN_DL} (9.6.3.2), got {len(frame)}")
        if pci_type(frame[0]) is not PciType.FIRST_FRAME:
            raise ValueError(f"not a FirstFrame: byte #1 is {frame[0]:#04x}")
        high = frame[0] & 0x0F
        if high == 0 and frame[1] == 0:
            raise ValueError("FirstFrame escape sequence (FF_DL > 4095, 32-bit) is a ROADMAP item")
        length = (high << 8) | frame[1]
        if length < FF_DL_MIN:
            raise ValueError(f"FF_DL {length} is below FF_DLmin {FF_DL_MIN} (9.6.3.2)")
        return cls(length, frame[2 : 2 + FF_DATA_LEN])


@dataclass(frozen=True, slots=True)
class ConsecutiveFrame:
    """CF N_PDU: a following frame of a segmented message (ISO 15765-2:2016, 9.6.4).

    ``data`` holds the payload bytes as read from the frame (up to 7); the last CF of a
    message may carry fewer meaningful bytes, which the reassembler trims by FF_DL.
    """

    sn: int
    data: bytes

    def __post_init__(self) -> None:
        if not 0 <= self.sn <= SN_MAX:
            raise ValueError(f"SequenceNumber must be 0..{SN_MAX}, got {self.sn}")
        if not 1 <= len(self.data) <= CF_DATA_MAX:
            raise ValueError(f"ConsecutiveFrame payload must be 1..{CF_DATA_MAX} bytes, got {len(self.data)}")

    def encode(self, *, pad: bool = True, padding: int = DEFAULT_PADDING) -> bytes:
        """The on-wire CF frame: ``0x2<SN>`` then the data, padded to 8 when ``pad``."""
        frame = bytes([(PciType.CONSECUTIVE_FRAME << 4) | self.sn]) + self.data
        return _pad(frame, padding) if pad else frame

    @classmethod
    def decode(cls, frame: bytes) -> ConsecutiveFrame:
        """Decode a CF frame into its SequenceNumber and payload bytes (padding included)."""
        _require_frame(frame)
        if pci_type(frame[0]) is not PciType.CONSECUTIVE_FRAME:
            raise ValueError(f"not a ConsecutiveFrame: byte #1 is {frame[0]:#04x}")
        if len(frame) < 2:
            raise ValueError("ConsecutiveFrame carries no data byte")
        return cls(frame[0] & 0x0F, frame[1:])


@dataclass(frozen=True, slots=True)
class FlowControl:
    """FC N_PDU: regulates the ConsecutiveFrame rate (ISO 15765-2:2016, 9.6.5).

    ``block_size`` is BS (0 = send all remaining CFs; 1..255 = CFs per block) and
    ``stmin`` is the raw STmin byte (parse it with ``flowcontrol.SeparationTime.from_byte``).
    """

    flow_status: FlowStatus
    block_size: int = 0
    stmin: int = 0

    def __post_init__(self) -> None:
        if not 0 <= self.block_size <= 0xFF:
            raise ValueError(f"BlockSize must be 0..255, got {self.block_size}")
        if not 0 <= self.stmin <= 0xFF:
            raise ValueError(f"STmin byte must be 0..255, got {self.stmin}")

    def encode(self, *, pad: bool = True, padding: int = DEFAULT_PADDING) -> bytes:
        """The on-wire FC frame: ``0x3<FS>`` ``<BS>`` ``<STmin>``, padded to 8 when ``pad``."""
        frame = bytes([(PciType.FLOW_CONTROL << 4) | int(self.flow_status), self.block_size, self.stmin])
        return _pad(frame, padding) if pad else frame

    @classmethod
    def decode(cls, frame: bytes) -> FlowControl:
        """Decode an FC frame, rejecting a reserved FlowStatus (N_INVALID_FS, 9.6.5.2)."""
        _require_frame(frame)
        if len(frame) < 3:
            raise ValueError(f"FlowControl needs 3 N_PCI bytes, got {len(frame)}")
        if pci_type(frame[0]) is not PciType.FLOW_CONTROL:
            raise ValueError(f"not a FlowControl: byte #1 is {frame[0]:#04x}")
        fs_nibble = frame[0] & 0x0F
        try:
            flow_status = FlowStatus(fs_nibble)
        except ValueError:
            raise ValueError(f"reserved FlowStatus {fs_nibble:#x} (N_INVALID_FS, 9.6.5.2)") from None
        return cls(flow_status, frame[1], frame[2])


NPdu = SingleFrame | FirstFrame | ConsecutiveFrame | FlowControl


def decode_frame(frame: bytes) -> NPdu:
    """Decode any classic-CAN ISO-TP frame into its N_PDU, dispatched on N_PCItype."""
    _require_frame(frame)
    kind = pci_type(frame[0])
    if kind is PciType.SINGLE_FRAME:
        return SingleFrame.decode(frame)
    if kind is PciType.FIRST_FRAME:
        return FirstFrame.decode(frame)
    if kind is PciType.CONSECUTIVE_FRAME:
        return ConsecutiveFrame.decode(frame)
    return FlowControl.decode(frame)
