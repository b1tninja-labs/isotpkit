# Obligations and their evidence

Every obligation `isotpkit` has taken on, with an honest status against the code.

**The honesty rule:** a row reads `[ok]` only when the callable it names exists and the test named in its
Evidence cell exists and would fail if the behaviour regressed. `tests/test_roadmap_rows.py` resolves every
`[ok]` row's test and every callable the matrix names.

Legend: `[ok]` implemented and pinned by a named test — `[part]` partial, what is missing stated — `[ ]` not
built — `[no]` deliberate non-goal, with the reason.

Every frame-format and parameter detail is grounded in ISO 15765-2:2016. The Spec column gives the clause; the
full citations are in `docs/iso-map.md`.

## 1. Matrix

### N_PDU frame codec (classic CAN, normal addressing)

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.pci.type` | `pci.pci_type()` classifies N_PCI byte #1 and rejects the reserved 0x4..0xF range | 9.6.1 | `[ok]` | `tests/test_pci.py::test_reserved_pci_type_is_rejected` |
| `it.sf.encode` | `pci.SingleFrame.encode()` builds the SF N_PCI byte and pads to 8 | 9.6.2 | `[ok]` | `tests/test_pci.py::test_single_frame_encodes_pci_and_pads` |
| `it.sf.decode` | `pci.SingleFrame.decode()` reads SF_DL and validates it against the frame | 9.6.2 | `[ok]` | `tests/test_pci.py::test_single_frame_decode_roundtrip` |
| `it.ff.encode` | `pci.FirstFrame.encode()` encodes the 12-bit FF_DL and the first 6 bytes | 9.6.3 | `[ok]` | `tests/test_pci.py::test_first_frame_encodes_12bit_length` |
| `it.ff.decode` | `pci.FirstFrame.decode()` reads the 12-bit FF_DL and rejects CAN_DL < 8 | 9.6.3 | `[ok]` | `tests/test_pci.py::test_first_frame_decode_roundtrip` |
| `it.cf.encode` | `pci.ConsecutiveFrame.encode()` builds the SN N_PCI byte | 9.6.4 | `[ok]` | `tests/test_pci.py::test_consecutive_frame_encode` |
| `it.cf.decode` | `pci.ConsecutiveFrame.decode()` reads the SequenceNumber and payload | 9.6.4 | `[ok]` | `tests/test_pci.py::test_consecutive_frame_decode` |
| `it.fc.encode` | `pci.FlowControl.encode()` builds FS, BS and the STmin byte | 9.6.5 | `[ok]` | `tests/test_pci.py::test_flow_control_encode` |
| `it.fc.decode` | `pci.FlowControl.decode()` parses FS (CTS/WAIT/OVFLW), BS, STmin and rejects a reserved FS | 9.6.5 | `[ok]` | `tests/test_pci.py::test_flow_control_decode` |

### FlowControl parameters

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.fc.stmin-ms` | `flowcontrol.SeparationTime.from_byte()` decodes the 0..127 ms STmin range | 9.6.5.4 | `[ok]` | `tests/test_flowcontrol.py::test_stmin_milliseconds` |
| `it.fc.stmin-us` | `flowcontrol.SeparationTime.from_byte()` decodes the 0xF1..0xF9 100..900 us range | 9.6.5.4 | `[ok]` | `tests/test_flowcontrol.py::test_stmin_microseconds` |
| `it.fc.stmin-reserved` | `flowcontrol.SeparationTime.from_byte()` maps a reserved STmin to the 127 ms fallback | 9.6.5.5 | `[ok]` | `tests/test_flowcontrol.py::test_reserved_stmin_falls_back` |
| `it.fc.stmin-encode` | `flowcontrol.encode_stmin()` is the inverse and refuses an unrepresentable value | 9.6.5.4 | `[ok]` | `tests/test_flowcontrol.py::test_encode_stmin_roundtrip` |

### Segmentation and reassembly

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.seg.single` | `segment.segment()` emits one SingleFrame for a message of 1..7 bytes | 9.3 | `[ok]` | `tests/test_segment.py::test_short_message_is_a_single_frame` |
| `it.seg.multi` | `segment.segment()` emits FF + CFs with SN starting at 1 | 9.3 | `[ok]` | `tests/test_segment.py::test_long_message_splits_into_ff_and_cfs` |
| `it.seg.wrap` | `segment.segment()` wraps the SequenceNumber 0xF -> 0x0 | 9.6.4.3 | `[ok]` | `tests/test_segment.py::test_sequence_number_wraps_past_15` |
| `it.reasm.roundtrip` | `segment.reassemble()` rebuilds the message from FF + ordered CFs | 9.3 | `[ok]` | `tests/test_segment.py::test_reassemble_round_trips_segmentation` |
| `it.reasm.sf` | `segment.Reassembler.feed()` returns an SF payload immediately | 9.6.2 | `[ok]` | `tests/test_segment.py::test_single_frame_reassembles_immediately` |
| `it.reasm.wrongsn` | `segment.Reassembler.feed()` rejects a wrong SequenceNumber (N_WRONG_SN) | 9.6.4.4 | `[ok]` | `tests/test_segment.py::test_wrong_sequence_number_is_rejected` |

### Addressing and timing

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.addr.11bit` | `addressing.NormalAddressing()` holds an 11-bit request/response pair and rejects wider IDs | 10.3.2 | `[ok]` | `tests/test_addressing.py::test_non_11bit_id_is_rejected` |
| `it.addr.gmlan-resp` | `addressing.gmlan_response_id()` offsets the response ID by +0x400 | volt appendix | `[ok]` | `tests/test_addressing.py::test_gmlan_response_offset` |
| `it.addr.gmlan-filter` | `addressing.gmlan_filter_id()` offsets the filter ID by +0x300 | volt appendix | `[ok]` | `tests/test_addressing.py::test_gmlan_filter_offset` |
| `it.timing.params` | `timing.parameter_names()` names N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr | 9.8.1 | `[ok]` | `tests/test_timing.py::test_timing_parameters_are_named` |

### Command line

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.cli.decode` | `cli.main()` decodes one frame and prints its N_PDU fields | 9.6 | `[ok]` | `tests/test_cli.py::test_cli_decode` |
| `it.cli.segment` | `cli.main()` segments a message into its frame sequence | 9.3 | `[ok]` | `tests/test_cli.py::test_cli_segment` |

## 2. Planned (not built)

These rows name no callable, so the matrix test does not require one; each is a deeper slice held for later.

| Id | Obligation | Spec | Status | Evidence |
|---|---|---|---|---|
| `it.canfd.frames` | CAN FD frames (CAN_DL > 8) and the receiver RX_DL determination | 6, 9.5 | `[ ]` | — |
| `it.sf.escape` | the SingleFrame escape (SF_DL > 7, second PCI byte) for CAN FD | 9.6.2 | `[ ]` | — |
| `it.ff.escape` | the 32-bit FirstFrame escape for FF_DL > 4095 | 9.6.3 | `[ ]` | — |
| `it.addr.extended` | extended, normal-fixed and mixed addressing (in-band address byte) | 10.3.3 | `[ ]` | — |
| `it.flow.pacing` | a FlowControl-paced sender/receiver loop (BS blocks, WAIT, N_WFTmax) | 9.6.5 | `[ ]` | — |
| `it.timing.timers` | a live timer loop for N_As..N_Cr (needs a bus) | 9.8.2 | `[no]` | out of scope: offline codec, no bus |
