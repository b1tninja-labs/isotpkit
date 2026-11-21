"""Segmentation and reassembly of an application message across ISO-TP N_PDUs.

Grounded in ISO 15765-2:2016, 9.3 (Multiple-frame transmission) and 9.6.4 (ConsecutiveFrame
SequenceNumber). Segmentation is pure (it emits the N_PDU sequence a sender would send,
without the FlowControl pacing that needs a live bus); reassembly validates SequenceNumbers.
"""

from __future__ import annotations

from collections.abc import Iterable

from isotpkit.pci import (
    CF_DATA_MAX,
    FF_DATA_LEN,
    FF_DL_12BIT_MAX,
    SF_DL_MAX,
    SN_MAX,
    ConsecutiveFrame,
    FirstFrame,
    SingleFrame,
)

SegmentPdu = SingleFrame | FirstFrame | ConsecutiveFrame


class ReassemblyError(ValueError):
    """A segmented message could not be reassembled (e.g. a wrong SequenceNumber)."""


def segment(message: bytes) -> list[SegmentPdu]:
    """Split an application message into its ISO-TP N_PDU sequence.

    A message of 1..7 bytes becomes one SingleFrame. A longer message becomes a FirstFrame
    (carrying the 12-bit length and the first 6 bytes) followed by ConsecutiveFrames whose
    SequenceNumber starts at 1 and wraps 0xF -> 0x0. Messages over 4095 bytes need the
    32-bit FF_DL escape, a ROADMAP item, and raise ``ValueError``.
    """
    if not message:
        raise ValueError("cannot segment an empty message")
    if len(message) <= SF_DL_MAX:
        return [SingleFrame(message)]
    if len(message) > FF_DL_12BIT_MAX:
        raise ValueError(
            f"message of {len(message)} bytes exceeds the 12-bit FF_DL max {FF_DL_12BIT_MAX}; "
            "the 32-bit FF_DL escape is a ROADMAP item"
        )
    pdus: list[SegmentPdu] = [FirstFrame(len(message), message[:FF_DATA_LEN])]
    sn = 1
    offset = FF_DATA_LEN
    while offset < len(message):
        pdus.append(ConsecutiveFrame(sn, message[offset : offset + CF_DATA_MAX]))
        sn = (sn + 1) & SN_MAX
        offset += CF_DATA_MAX
    return pdus


class Reassembler:
    """Consume an ordered N_PDU sequence back into the application message.

    ``feed`` returns the completed message once the last frame arrives, otherwise ``None``.
    A SingleFrame completes immediately; a FirstFrame opens a session and ConsecutiveFrames
    fill it in order. A ConsecutiveFrame with the wrong SequenceNumber raises
    ``ReassemblyError`` (the N_WRONG_SN condition of 9.6.4.4).
    """

    def __init__(self) -> None:
        self._expected: int | None = None
        self._buffer = bytearray()
        self._next_sn = 1

    @property
    def in_progress(self) -> bool:
        """Whether a FirstFrame has opened a message that is not yet complete."""
        return self._expected is not None

    def reset(self) -> None:
        """Discard any in-progress reassembly state."""
        self._expected = None
        self._buffer = bytearray()
        self._next_sn = 1

    def feed(self, pdu: SegmentPdu) -> bytes | None:
        """Feed the next N_PDU; return the message if complete, else ``None``."""
        if isinstance(pdu, SingleFrame):
            if self._expected is not None:
                raise ReassemblyError("SingleFrame arrived during an in-progress reassembly")
            return bytes(pdu.data)
        if isinstance(pdu, FirstFrame):
            if self._expected is not None:
                raise ReassemblyError("FirstFrame arrived during an in-progress reassembly")
            self._expected = pdu.length
            self._buffer = bytearray(pdu.data)
            self._next_sn = 1
            return None
        if self._expected is None:
            raise ReassemblyError("ConsecutiveFrame arrived before any FirstFrame")
        if pdu.sn != self._next_sn:
            raise ReassemblyError(f"wrong SequenceNumber: expected {self._next_sn}, got {pdu.sn} (N_WRONG_SN, 9.6.4.4)")
        remaining = self._expected - len(self._buffer)
        self._buffer.extend(pdu.data[:remaining])
        self._next_sn = (self._next_sn + 1) & SN_MAX
        if len(self._buffer) >= self._expected:
            message = bytes(self._buffer[: self._expected])
            self.reset()
            return message
        return None


def reassemble(pdus: Iterable[SegmentPdu]) -> bytes:
    """Reassemble a complete N_PDU sequence into its message, or raise ``ReassemblyError``."""
    reassembler = Reassembler()
    message: bytes | None = None
    for pdu in pdus:
        message = reassembler.feed(pdu)
    if message is None:
        raise ReassemblyError("the N_PDU sequence did not complete a message")
    return message
