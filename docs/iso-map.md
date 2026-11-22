# ISO 15765-2:2016 clause map

Every frame-format and parameter detail in `isotpkit` is grounded in ISO 15765-2:2016, *Road vehicles —
Diagnostic communication over Controller Area Network (DoCAN) — Part 2: Transport protocol and network layer
services*, read through the `primarysource` `iso_*` tools on `iso:15765-2@2016`. This file maps each piece of
the implementation to the clause it comes from.

Clause numbers are the deepest the `iso_*` tools cite (three levels, e.g. §9.6.4); finer paragraph labels such
as 9.6.4.3 live inside those sections and are named in prose where they matter.

## N_PCI and the N_PDU types

| Detail | Clause | Where in code |
|---|---|---|
| N_PCItype bit values: SF=0x0, FF=0x1, CF=0x2, FC=0x3; 0x4..0xF reserved | §9.6.1, Table 8 | `pci::PciType`, `pci::PciType::from_byte1` |
| Summary of N_PCI bytes per N_PDU | §9.6.1, Table 9 | `pci` encoders/decoders |
| SingleFrame: SF_DL in the low nibble of byte #1; SF_DL 1..7 (7 normal addressing only); CAN_DL>8 escape uses low nibble 0 | §9.6.2, Tables 10–13 | `pci::SingleFrame` |
| SF_DL error handling: ignore SF_DL 0, or SF_DL greater than the available data bytes | §9.6.2 | `pci::SingleFrame::decode` |
| FirstFrame: 12-bit FF_DL (MSB = bit 3 of byte #1, LSB = bit 0 of byte #2); FF_DLmin = 8 (normal, TX_DL=8); 32-bit escape for FF_DL>4095 | §9.6.3, Tables 14–15 | `pci::FirstFrame`, `pci::FF_DL_MIN`, `pci::FF_DL_12BIT_MAX` |
| FF error handling: CAN_DL < 8 ignored; FF_DL < FF_DLmin ignored | §9.6.3 | `pci::FirstFrame::decode` |
| ConsecutiveFrame: SN in the low nibble of byte #1; FF is segment 0, first CF is SN 1, increments, wraps 0xF->0x0 (9.6.4.3) | §9.6.4, Tables 16–17 | `pci::ConsecutiveFrame`, `segment::segment` |
| CF SequenceNumber error handling (N_WRONG_SN, 9.6.4.4) | §9.6.4 | `segment::Reassembler::feed` |
| FlowControl: FS in the low nibble of byte #1 — CTS=0, WAIT=1, OVFLW=2; 3..F reserved | §9.6.5, Table 18 | `flowcontrol::FlowStatus`, `pci::FlowControl::decode` |
| Reserved FS -> N_INVALID_FS (9.6.5.2) | §9.6.5 | `pci::FlowControl::decode` |
| BlockSize (BS) in byte #2: 0 = send all remaining CFs; 1..255 = CFs per block | §9.6.5, Table 19 | `pci::FlowControl::block_size` |
| STmin in byte #3: 0x00..0x7F = 0..127 ms; 0xF1..0xF9 = 100..900 us; others reserved | §9.6.5, Table 20 | `flowcontrol::SeparationTime`, `flowcontrol::encode_stmin` |
| Reserved STmin -> use 0x7F (127 ms) (9.6.5.5) | §9.6.5 | `flowcontrol::RESERVED_STMIN_FALLBACK_US` |

## Framing, padding and addressing

| Detail | Clause | Where in code |
|---|---|---|
| Multiple-frame transmission: FF then CFs | §9.3 | `segment::segment`, `segment::reassemble` |
| Default frame padding value 0xCC (TX_DL = 8, 10.4.2.1) | §10.4.2 | `pci::DEFAULT_PADDING` |
| CAN frame data optimization (shortened DLC, no padding, 10.4.2.2) | §10.4.2 | `pci::*::encode_with(false, …)` |
| Normal addressing: whole N_AI is the CAN identifier; data field is N_PCI + N_Data | §10.3.2, Tables 24–25 | `pci` (no in-band address byte), `addressing::NormalAddressing` |
| Extended addressing: N_TA in the first data byte, then N_PCI + N_Data | §10.3.4, Tables 28–29 | `pci::Addressing::Extended`, `pci::encode_addressed`/`decode_addressed` |
| Mixed addressing: N_AE in the first data byte, then N_PCI + N_Data | §10.3.5, Tables 30–33 | `pci::Addressing::Mixed` |
| Normal-fixed 29-bit CAN ID: 0x18DA (physical) / 0x18DB (functional), N_TA in 15–8, N_SA in 7–0 | §10.3.3, Tables 26–27 | `addressing::normal_fixed_id` |
| Mixed 29-bit CAN ID: 0x18CE (physical) / 0x18CD (functional), N_TA in 15–8, N_SA in 7–0 | §10.3.5, Tables 30–31 | `addressing::mixed_id_29bit` |
| FF_DLmin for TX_DL=8: 8 (normal), 7 (extended/mixed) | §9.6.3, Table 14 | `pci::FF_DL_MIN`, `pci::FF_DL_MIN_EXTENDED` |

## Timing (named only — no bus, no timer loop)

| Detail | Clause | Where in code |
|---|---|---|
| Timing parameters N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr; timeouts 1000 ms; N_Br/N_Cs performance requirements | §9.8.1, Table 21 | `timing::PARAMETERS`, `timing::TimingParameter::timeout_ms` |
| Real timeout no later than the specified value + 50 % | §9.8.1 | `timing::TIMEOUT_TOLERANCE` |
| N_WFTmax: max consecutive FC.WAIT, local, not on the wire; 0 => CTS only | §9.7 | `timing` module note |

## CAN FD framing (implemented)

Grounded additions from the CAN FD work; these are built in `pci` and `segment`.

| Detail | Clause | Where in code |
|---|---|---|
| Valid `CAN_DL` / `RX_DL` values: 8 (classic), 12/16/20/24/32/48/64 (CAN FD); RX_DL = the FF's CAN_DL | §9.5.4, Table 7 | `pci::VALID_CAN_DL`, `pci::decode_frame` |
| TX_DL configuration values 8 and 12/16/20/24/32/48/64 (CAN_DL ≤ TX_DL), driving SF/FF/CF sizing | §9.5.1, Table 6 | `segment::segment_with_tx_dl` |
| SingleFrame escape (CAN_DL > 8): byte #1 low nibble 0, SF_DL in byte #2, 8 ≤ SF_DL ≤ CAN_DL − 2 (normal) | §9.6.2, Tables 11, 13 | `pci::SingleFrame` |
| FirstFrame 32-bit escape (FF_DL > 4095): byte #1 low nibble 0, byte #2 0, FF_DL in bytes #3 (MSB)..#6 (LSB) | §9.6.3, Tables 14, 15 | `pci::FirstFrame` |
| ConsecutiveFrame carries up to CAN_DL − 1 bytes on CAN FD | §9.6.4 | `pci::ConsecutiveFrame` |
| Mandatory padding of CAN FD frames to the next valid CAN_DL (value 0xCC) | §10.4.2.3 | `pci` encoders |

## FlowControl pacing (implemented, sans-I/O)

The `session` module paces a sender/receiver by emitting actions; it owns no bus and runs no timer.

| Detail | Clause | Where in code |
|---|---|---|
| CTS/BS/STmin: after a FlowControl CTS the sender sends up to BlockSize ConsecutiveFrames, spaced by STmin; BS = 0 sends all | §9.6.5 | `session::TxSession` |
| FC.WAIT keeps the sender waiting (restart N_Bs); FC.Overflow aborts with N_BUFFER_OVFLW | §9.6.5 | `session::TxSession::on_flow_control` |
| The receiver sends a FlowControl after the FirstFrame and after each block of BlockSize CFs; Overflow when FF_DL exceeds the buffer | §9.6.5 | `session::RxSession::on_frame` |
| N_WFTmax: the receiver may send at most that many FC.WAIT in a row, then N_WFT_OVRN | §9.7, §9.8.4 | `session::RxSession::request_wait` |
| Unexpected N_PDU (Table 23): ignore a stray CF/FC/unknown; a mid-reception SF/FF terminates the prior with N_UNEXP_PDU; run one session per N_AI | §9.8.3 | `session::RxSession::on_frame` |
| Receive-side CAN_DL verification: RX_DL is the FF's CAN_DL; every CF must match it, only the last may be shorter (and must carry the remaining bytes) | §9.5.3, §9.6.4.1 | `session::RxSession::consecutive_can_dl_ok` |
| DLC error handling: a CF whose CAN_DL is smaller than the N_PCI implies is ignored without further action | §10.4.3 | `session::RxSession::consecutive_can_dl_ok` |

## Network-layer service primitives (implemented)

The §8 service interface the network layer offers the layer above it, as sans-I/O data types (the
primitives and their SDU parameters); no bus and no timer are run here.

| Detail | Clause | Where in code |
|---|---|---|
| The six service primitives: N_USData.request/.confirm, N_USData_FF.indication, N_USData.indication, N_ChangeParameter.request/.confirm | §8.2.1–§8.2.6 | `service::Primitive` |
| Mtype (diagnostics / remote diagnostics) selecting whether N_AE is part of N_AI | §8.3.1 | `service::Mtype`, `service::AddressInfo` |
| N_AI: N_SA, N_TA, N_TAtype (physical/functional; functional is SingleFrame-only), N_AE | §8.3.2 | `service::AddressInfo`, `service::TargetAddressType` |
| `<Length>` (32-bit, ≥ 1) and `<MessageData>` (valid only when N_Result = N_OK on an indication) | §8.3.3, §8.3.4 | `service::Primitive::us_data_request`, `service::Primitive::message_data` |
| `<Parameter>` (STmin, BS) and `<Parameter_Value>` (8-bit) | §8.3.5, §8.3.6 | `service::Parameter`, `service::Primitive::ChangeParameterRequest` |
| `<Result_ChangeParameter>` (N_OK, N_RX_ON, N_WRONG_PARAMETER, N_WRONG_VALUE); a change is refused (N_RX_ON) from the FF until the end of reception | §8.3.8, §8.2.5 | `service::ResultChangeParameter`, `service::change_parameter_result` |

## `<N_Result>` outcome codes (implemented)

The §8.3.7 enumeration the `N_USData` service primitives report, with the two rules the clause
states about it. The crate runs no timer, so the three timeout codes are named but not produced
from the crate's own `Error`.

| Detail | Clause | Where in code |
|---|---|---|
| The `<N_Result>` enumeration: N_OK, N_TIMEOUT_A/Bs/Cr, N_WRONG_SN, N_INVALID_FS, N_UNEXP_PDU, N_WFT_OVRN, N_BUFFER_OVFLW, N_ERROR | §8.3.7 | `result::NResult` |
| Priority ordering — the value found first in the list wins when several errors are discovered at once | §8.3.7 | `result::NResult::rank`, `result::NResult::first_in_list`, `result::NResult::earliest` |
| The side each value can be issued to (sender only / receiver only / both) | §8.3.7 | `result::NResult::side` |
| Mapping the crate's own decode/encode errors onto their `<N_Result>` value; anything without a dedicated code is N_ERROR | §8.3.7 | `result::NResult::from` |

## Spec surface not yet implemented (ROADMAP `[open]` rows)

The clauses below are part of ISO 15765-2:2016 but are not implemented in `isotpkit` today; each is a
`[open]` row in `ROADMAP.md`. They are recorded here, grounded, so the matrix is honest about the
remaining spec, and so a later implementation has the clause to read first.

| Detail | Clause | Status |
|---|---|---|
| Verifying a received frame's CAN type by N_TAtype (CLASSICAL CAN vs CAN FD filtering) | §9.5.2–§9.5.3 | planned (the receive-side CAN_DL/RX_DL check of §9.6.4.1 is built; frame-type filtering needs the CAN frame's type, which the sans-I/O byte slice does not carry) |
| Interleaving: parallel transmission/reassembly of messages on distinct N_AI | §9.9 | planned |
| ISO-reserved 11-bit CAN IDs: 0x7F4–0x7F6 and 0x7FA–0x7FB | §B (Annex B), Table B.1 | planned |

## Citation

Clause text was read with `iso_read` / `iso_cite` on `iso:15765-2@2016`. Canonical form, e.g.:

> ISO 15765-2:2016, Road vehicles — Diagnostic communication over Controller Area Network (DoCAN) —
> Part 2: Transport protocol and network layer services, §9.6.5 FlowControl N_PCI parameter definition
