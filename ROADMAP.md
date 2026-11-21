# isotpkit roadmap

`isotpkit` is the ISO-TP (ISO 15765-2:2016, DoCAN) transport/network-layer codec, in Rust. It
segments a UDS/GMLAN application message into classic-CAN frames and reassembles it, encodes and
decodes the four N_PDU types, and names the network-layer timing parameters. It is sans-I/O: no
bus, no sockets, no live timer loop. It sits *under* an application-layer UDS codec such as the
sibling `udsdiag` and does not depend on one.

Rows are `Status | Obligation | Anchor | Test`, the grammar `projecthub sync` reads for Rust
projects (`projecthub/docs/rust-support.md`). The Test cell names a Rust test; `cargo test` runs
them.

Status:

- `[ok]` the named test in this crate exists and passed on 2026-11-21, and would fail if the
  behaviour regressed.
- `[open]` not built here; the decoder rejects the frame rather than guessing, and the Test cell
  names the rejection test where one exists.
- `[no]` a deliberate non-goal. The Test cell stays empty of a test name.

Every frame-format and parameter detail is grounded in ISO 15765-2:2016, read through the
`primarysource` `iso_*` tools; the clause-by-clause map is `docs/iso-map.md`.

## Registration

Measured 2026-11-21 from `D:\code\projecthub` and `D:\code\PORTFOLIO.md`. This repository does not
edit either file.

| Field | Recorded value |
|---|---|
| Registry entry | `isotpkit` in `projecthub/src/projecthub/registry.py` |
| Registered | 2026-11-21, `PORTFOLIO.md` section 3, untiered and unsequenced |
| GitHub | <https://github.com/b1tninja-labs/isotpkit> |
| Matrix format | Rust (`Status \| Obligation \| Anchor \| Test`), as of this port from Python |
| `RUST_OWNED` | **absent** — still listed in projecthub's Python/hand-owned set. The HAND_OWNED→RUST_OWNED move is handled separately (the `register-vehicle-tools` session), not from here |
| Sync caveat | until that move, `projecthub sync --project isotpkit` parses this file with the Python matrix reader, which finds no `[id]` rows. Issues `b1tninja-labs/portfolio#1135`–`#1140` were filed from the *previous* Python matrix and are reconciled by the move + a re-sync, not by this repo |

## N_PDU frame codec (classic CAN, normal addressing)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `pci::PciType::from_byte1` classifies N_PCI byte #1 and rejects the reserved 0x4..=0xF range | `docs/iso-map.md` §9.6.1; `src/pci.rs` | `pci::tests::reserved_pci_type_is_rejected` |
| [ok] | `pci::SingleFrame::encode` builds the SF N_PCI byte and pads to 8, and `encode_with(false, …)` omits padding | `docs/iso-map.md` §9.6.2, §10.4.2 | `pci::tests::single_frame_encodes_pci_and_pads` |
| [ok] | `pci::SingleFrame::decode` reads SF_DL, validates it against the frame, and rejects the SF_DL 0 escape | `docs/iso-map.md` §9.6.2 | `pci::tests::single_frame_decode_roundtrip` |
| [ok] | `pci::FirstFrame::encode` encodes the 12-bit FF_DL and the first 6 bytes | `docs/iso-map.md` §9.6.3 | `pci::tests::first_frame_encodes_12bit_length` |
| [ok] | `pci::FirstFrame::decode` reads the 12-bit FF_DL and rejects CAN_DL < 8, the 32-bit escape and FF_DL < FF_DLmin | `docs/iso-map.md` §9.6.3 | `pci::tests::first_frame_decode_roundtrip_and_rejects_short` |
| [ok] | `pci::ConsecutiveFrame::encode`/`decode` carry the SequenceNumber and payload | `docs/iso-map.md` §9.6.4 | `pci::tests::consecutive_frame_encode_and_decode` |
| [ok] | `pci::FlowControl::encode` builds FS, BS and the STmin byte | `docs/iso-map.md` §9.6.5 | `pci::tests::flow_control_encode_and_decode` |
| [ok] | `pci::FlowControl::decode` parses FS (CTS/WAIT/OVFLW), BS and STmin, and rejects a reserved FS (N_INVALID_FS) | `docs/iso-map.md` §9.6.5 | `pci::tests::flow_control_encode_and_decode` |
| [ok] | `pci::decode_frame` dispatches on N_PCItype and rejects empty and over-length frames | `src/pci.rs` | `pci::tests::decode_frame_dispatches_on_type` |

## FlowControl parameters

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `flowcontrol::SeparationTime::from_byte` decodes the 0..=127 ms STmin range | `docs/iso-map.md` §9.6.5 | `flowcontrol::tests::stmin_milliseconds` |
| [ok] | `flowcontrol::SeparationTime::from_byte` decodes the 0xF1..=0xF9 100..=900 µs range | `docs/iso-map.md` §9.6.5 | `flowcontrol::tests::stmin_microseconds` |
| [ok] | `flowcontrol::SeparationTime::from_byte` maps a reserved STmin to the 127 ms fallback | `docs/iso-map.md` §9.6.5 | `flowcontrol::tests::reserved_stmin_falls_back` |
| [ok] | `flowcontrol::encode_stmin` is the inverse and refuses an unrepresentable value | `docs/iso-map.md` §9.6.5 | `flowcontrol::tests::encode_stmin_roundtrip_and_rejects` |

## Segmentation and reassembly

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `segment::segment` emits one SingleFrame for a message of 1..=7 bytes, and rejects empty | `docs/iso-map.md` §9.3 | `segment::tests::short_message_is_a_single_frame` |
| [ok] | `segment::segment` emits FF + CFs with the SequenceNumber starting at 1 | `docs/iso-map.md` §9.3 | `segment::tests::long_message_splits_into_ff_and_cfs` |
| [ok] | `segment::segment` wraps the SequenceNumber 0xF → 0x0 | `docs/iso-map.md` §9.6.4 | `segment::tests::sequence_number_wraps_past_15` |
| [ok] | `segment::reassemble` rebuilds the message from FF + ordered CFs for every length 1..=4095 | `docs/iso-map.md` §9.3 | `segment::tests::reassemble_round_trips_segmentation` |
| [ok] | `segment::Reassembler::feed` returns an SF payload immediately | `docs/iso-map.md` §9.6.2 | `segment::tests::single_frame_reassembles_immediately` |
| [ok] | `segment::Reassembler::feed` rejects a wrong SequenceNumber (N_WRONG_SN) and a CF with no FirstFrame | `docs/iso-map.md` §9.6.4 | `segment::tests::wrong_sequence_number_is_rejected` |

## Addressing and timing

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `addressing::NormalAddressing::new` holds an 11-bit request/response pair and rejects wider IDs | `docs/iso-map.md` §10.3.2 | `addressing::tests::non_11bit_id_is_rejected` |
| [ok] | `addressing::gmlan_response_id` offsets the response ID by +0x400 | `docs/sources.md` (volt appendix) | `addressing::tests::gmlan_response_offset` |
| [ok] | `addressing::gmlan_filter_id` offsets the filter ID by +0x300 | `docs/sources.md` (volt appendix) | `addressing::tests::gmlan_filter_offset` |
| [ok] | `timing::parameter_names` names N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr; `TimingParameter::timeout_ms` carries the Table 21 timeouts | `docs/iso-map.md` §9.8.1 | `timing::tests::timing_parameters_are_named` |

## Command line and crate shape

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [ok] | `cli::run` decodes one frame and describes its N_PDU fields | `src/cli.rs` | `cli::tests::cli_decode` |
| [ok] | `cli::run` segments a message into its frame sequence | `src/cli.rs` | `cli::tests::cli_segment` |
| [ok] | `cli::describe` renders a FlowControl with its resolved STmin, and `run` reports usage/bad-hex errors | `src/cli.rs` | `cli::tests::describe_flow_control` |
| [ok] | The library is `#![no_std]` + `alloc`; only `src/main.rs` uses `std`. A `std` leak fails the no_std build | `src/lib.rs`; `Cargo.toml` | `compile`: `cargo check` builds the `not(test)` `#![no_std]` lib target |

## Planned (decoder rejects, not built)

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [open] | CAN FD frames (CAN_DL > 8) and the receiver RX_DL determination | `docs/iso-map.md` §6, §9.5 | none: not built; the decoder rejects CAN_DL > 8 (`pci::tests::decode_frame_dispatches_on_type`) |
| [open] | The SingleFrame escape (SF_DL > 7, second PCI byte) for CAN FD | `docs/iso-map.md` §9.6.2 | none: not built; the decoder rejects SF_DL 0 (`pci::tests::single_frame_decode_roundtrip`) |
| [open] | The 32-bit FirstFrame escape for FF_DL > 4095 | `docs/iso-map.md` §9.6.3 | none: not built; the decoder rejects the escape (`pci::tests::first_frame_decode_roundtrip_and_rejects_short`) |
| [open] | Extended, normal-fixed and mixed addressing (in-band address byte) | `docs/iso-map.md` §10.3.3–§10.3.5 | none: only normal addressing is built |
| [open] | A FlowControl-paced sender/receiver loop (BS blocks, WAIT, N_WFTmax) | `docs/iso-map.md` §9.6.5 | none: needs a bus; this crate is sans-I/O |
| [open] | Consolidate with `fakeobd`'s `isotp` crate into one canonical ISO-TP (see Overlap) | `docs/sources.md` | none: a cross-repo decision, deferred |

## Non-goals

| Status | Obligation | Anchor | Test |
|---|---|---|---|
| [no] | A live timer loop for N_As..N_Cr. Out of scope: this is an offline codec with no bus | `src/timing.rs`; `AGENTS.md` | none: the timers are not built |

## Overlap with fakeobd

`fakeobd/crates/isotp` already implements an ISO-TP codec, geared to the fake-vehicle server and
marked `[unread]` in its own ROADMAP (its PCI nibbles, FlowStatus and STmin were not re-read
against §9.6). `isotpkit` is the standalone, fully-grounded, sans-I/O crate: typed N_PDUs with
`encode`/`decode`, a `SeparationTime` and timing/addressing constants, and `no_std` support.

Long-term these should collapse to one canonical ISO-TP crate that `fakeobd` depends on. That is a
cross-repo decision (which crate name survives, whose API, how `fakeobd` migrates) and is left as
the `[open]` consolidation row above, not taken here.
