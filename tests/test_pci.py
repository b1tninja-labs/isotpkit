"""The four N_PDU types encode and decode on classic-CAN frames (ISO 15765-2:2016, 9.6)."""

from __future__ import annotations

import pytest

from isotpkit import pci
from isotpkit.flowcontrol import FlowStatus
from isotpkit.pci import (
    ConsecutiveFrame,
    FirstFrame,
    FlowControl,
    PciType,
    SingleFrame,
    decode_frame,
    pci_type,
)


def test_single_frame_encodes_pci_and_pads():
    frame = SingleFrame(b"\x22\xf1\x90").encode()
    assert frame == b"\x03\x22\xf1\x90\xcc\xcc\xcc\xcc"
    assert frame[0] >> 4 == PciType.SINGLE_FRAME
    assert frame[0] & 0x0F == 3  # SF_DL


def test_single_frame_decode_roundtrip():
    sf = SingleFrame(b"\x01\x02\x03\x04\x05")
    assert SingleFrame.decode(sf.encode()).data == sf.data
    # Padding past SF_DL is ignored on decode.
    assert SingleFrame.decode(b"\x02\xaa\xbb\xcc\xcc\xcc\xcc\xcc").data == b"\xaa\xbb"


def test_single_frame_rejects_escape_and_overlong_sf_dl():
    with pytest.raises(ValueError, match="escape"):
        SingleFrame.decode(b"\x00\x11\x22\x33\x44\x55\x66\x77")  # SF_DL 0 => CAN FD escape
    with pytest.raises(ValueError, match="exceeds"):
        SingleFrame.decode(b"\x07\x11\x22")  # claims 7 data bytes, only 2 present


def test_first_frame_encodes_12bit_length():
    # FF_DL = 4095 => high nibble 0xF, low byte 0xFF.
    frame = FirstFrame(4095, b"\x10\x20\x30\x40\x50\x60").encode()
    assert frame == b"\x1f\xff\x10\x20\x30\x40\x50\x60"
    assert frame[0] >> 4 == PciType.FIRST_FRAME


def test_first_frame_decode_roundtrip():
    ff = FirstFrame(100, bytes(range(6)))
    decoded = FirstFrame.decode(ff.encode())
    assert decoded.length == 100
    assert decoded.data == bytes(range(6))


def test_first_frame_rejects_escape_and_short_frame():
    with pytest.raises(ValueError, match="escape"):
        FirstFrame.decode(b"\x10\x00\x00\x00\x10\x00\x00\x00")  # 32-bit escape
    with pytest.raises(ValueError, match="CAN_DL"):
        FirstFrame.decode(b"\x10\x40\x11\x22")  # CAN_DL < 8


def test_consecutive_frame_encode():
    frame = ConsecutiveFrame(1, b"\x61\x62\x63\x64\x65\x66\x67").encode(pad=False)
    assert frame == b"\x21\x61\x62\x63\x64\x65\x66\x67"
    assert frame[0] >> 4 == PciType.CONSECUTIVE_FRAME
    assert frame[0] & 0x0F == 1  # SN


def test_consecutive_frame_decode():
    cf = ConsecutiveFrame.decode(b"\x2f\xde\xad\xbe\xef\x00\x00\x00")
    assert cf.sn == 0xF
    assert cf.data == b"\xde\xad\xbe\xef\x00\x00\x00"


def test_flow_control_encode():
    frame = FlowControl(FlowStatus.CONTINUE_TO_SEND, block_size=8, stmin=20).encode()
    assert frame == b"\x30\x08\x14\xcc\xcc\xcc\xcc\xcc"
    assert frame[0] >> 4 == PciType.FLOW_CONTROL


def test_flow_control_decode():
    fc = FlowControl.decode(b"\x31\x00\x00\xcc\xcc\xcc\xcc\xcc")
    assert fc.flow_status is FlowStatus.WAIT
    fc_cts = FlowControl.decode(b"\x30\x0a\xf1")
    assert fc_cts.flow_status is FlowStatus.CONTINUE_TO_SEND
    assert fc_cts.block_size == 10
    assert fc_cts.stmin == 0xF1


def test_flow_control_rejects_reserved_flow_status():
    with pytest.raises(ValueError, match="N_INVALID_FS"):
        FlowControl.decode(b"\x35\x00\x00")


def test_reserved_pci_type_is_rejected():
    with pytest.raises(ValueError, match="reserved N_PCItype"):
        pci_type(0x40)
    assert pci_type(0x10) is PciType.FIRST_FRAME


def test_decode_frame_dispatches_on_pci_type():
    assert isinstance(decode_frame(b"\x02\x11\x22\xcc\xcc\xcc\xcc\xcc"), SingleFrame)
    assert isinstance(decode_frame(b"\x10\x14\x00\x01\x02\x03\x04\x05"), FirstFrame)
    assert isinstance(decode_frame(b"\x21\x06\x07\x08\x09\x0a\x0b\x0c"), ConsecutiveFrame)
    assert isinstance(decode_frame(b"\x30\x00\x00\xcc\xcc\xcc\xcc\xcc"), FlowControl)


def test_classic_can_rejects_oversized_frame():
    with pytest.raises(ValueError, match="CAN FD"):
        pci.decode_frame(bytes(9))
