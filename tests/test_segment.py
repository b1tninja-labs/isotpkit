"""Segmentation and reassembly round-trips and SequenceNumber validation (9.3, 9.6.4)."""

from __future__ import annotations

import pytest

from isotpkit.pci import ConsecutiveFrame, FirstFrame, SingleFrame
from isotpkit.segment import Reassembler, ReassemblyError, reassemble, segment


def test_short_message_is_a_single_frame():
    pdus = segment(b"\x3e\x00")
    assert len(pdus) == 1
    assert isinstance(pdus[0], SingleFrame)
    assert pdus[0].data == b"\x3e\x00"


def test_long_message_splits_into_ff_and_cfs():
    message = bytes(range(20))  # 20 bytes => FF(6) + CF(7) + CF(7)
    pdus = segment(message)
    assert isinstance(pdus[0], FirstFrame)
    assert pdus[0].length == 20
    assert pdus[0].data == bytes(range(6))
    cfs = pdus[1:]
    assert [cf.sn for cf in cfs] == [1, 2]
    assert isinstance(cfs[0], ConsecutiveFrame)
    assert cfs[0].data == bytes(range(6, 13))
    assert cfs[-1].data == bytes(range(13, 20))


def test_sequence_number_wraps_past_15():
    # 6 (FF) + 15 CFs of 7 bytes reach SN 15, the 16th CF wraps to SN 0.
    message = bytes(6 + 7 * 16)
    pdus = segment(message)
    sns = [pdu.sn for pdu in pdus if isinstance(pdu, ConsecutiveFrame)]
    assert sns[:16] == [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0]


def test_reassemble_round_trips_segmentation():
    for size in (1, 7, 8, 13, 100, 4095):
        message = bytes((i * 7) & 0xFF for i in range(size))
        assert reassemble(segment(message)) == message


def test_single_frame_reassembles_immediately():
    reassembler = Reassembler()
    assert reassembler.feed(SingleFrame(b"\xaa\xbb")) == b"\xaa\xbb"
    assert reassembler.in_progress is False


def test_wrong_sequence_number_is_rejected():
    reassembler = Reassembler()
    assert reassembler.feed(FirstFrame(20, bytes(range(6)))) is None
    assert reassembler.feed(ConsecutiveFrame(1, bytes(range(6, 13)))) is None
    with pytest.raises(ReassemblyError, match="N_WRONG_SN"):
        reassembler.feed(ConsecutiveFrame(3, bytes(range(13, 20))))  # expected SN 2


def test_consecutive_frame_before_first_frame_is_rejected():
    with pytest.raises(ReassemblyError, match="before any FirstFrame"):
        Reassembler().feed(ConsecutiveFrame(1, b"\x00"))


def test_segment_rejects_empty_and_oversized():
    with pytest.raises(ValueError, match="empty"):
        segment(b"")
    with pytest.raises(ValueError, match="ROADMAP item"):
        segment(bytes(4096))
