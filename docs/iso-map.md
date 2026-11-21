# ISO 15765-2:2016 clause map

Every frame-format and parameter detail in `isotpkit` is grounded in ISO 15765-2:2016, *Road vehicles —
Diagnostic communication over Controller Area Network (DoCAN) — Part 2: Transport protocol and network layer
services*, read through the `primarysource` `iso_*` tools on `iso:15765-2@2016`. This file maps each piece of
the implementation to the clause it comes from.

## N_PCI and the N_PDU types

| Detail | Clause | Where in code |
|---|---|---|
| N_PCItype bit values: SF=0x0, FF=0x1, CF=0x2, FC=0x3; 0x4..0xF reserved | §9.6.1, Table 8 | `pci.PciType`, `pci.pci_type` |
| Summary of N_PCI bytes per N_PDU | §9.6.1, Table 9 | `pci` encoders/decoders |
| SingleFrame: SF_DL in the low nibble of byte #1; SF_DL 1..7 (7 normal addressing only); CAN_DL>8 escape uses low nibble 0 | §9.6.2, Tables 10–13 | `pci.SingleFrame` |
| SF_DL error handling: ignore SF_DL 0, or SF_DL greater than the available data bytes | §9.6.2.2 | `pci.SingleFrame.decode` |
| FirstFrame: 12-bit FF_DL (MSB = bit 3 of byte #1, LSB = bit 0 of byte #2); FF_DLmin = 8 (normal, TX_DL=8); 32-bit escape for FF_DL>4095 | §9.6.3, Tables 14–15 | `pci.FirstFrame`, `FF_DL_MIN`, `FF_DL_12BIT_MAX` |
| FF error handling: CAN_DL < 8 ignored; FF_DL < FF_DLmin ignored | §9.6.3.2 | `pci.FirstFrame.decode` |
| ConsecutiveFrame: SN in the low nibble of byte #1; FF is segment 0, first CF is SN 1, increments, wraps 0xF->0x0 | §9.6.4.3, Tables 16–17 | `pci.ConsecutiveFrame`, `segment.segment` |
| CF SequenceNumber error handling (N_WRONG_SN) | §9.6.4.4 | `segment.Reassembler.feed` |
| FlowControl: FS in the low nibble of byte #1 — CTS=0, WAIT=1, OVFLW=2; 3..F reserved | §9.6.5.1, Table 18 | `flowcontrol.FlowStatus`, `pci.FlowControl.decode` |
| Reserved FS -> N_INVALID_FS | §9.6.5.2 | `pci.FlowControl.decode` |
| BlockSize (BS) in byte #2: 0 = send all remaining CFs; 1..255 = CFs per block | §9.6.5.3, Table 19 | `pci.FlowControl.block_size` |
| STmin in byte #3: 0x00..0x7F = 0..127 ms; 0xF1..0xF9 = 100..900 us; others reserved | §9.6.5.4, Table 20 | `flowcontrol.SeparationTime`, `flowcontrol.encode_stmin` |
| Reserved STmin -> use 0x7F (127 ms) | §9.6.5.5 | `flowcontrol.RESERVED_STMIN_FALLBACK_US` |

## Framing, padding and addressing

| Detail | Clause | Where in code |
|---|---|---|
| Multiple-frame transmission: FF then CFs | §9.3 | `segment.segment`, `segment.reassemble` |
| Default frame padding value 0xCC (TX_DL = 8) | §10.4.2.1 | `pci.DEFAULT_PADDING` |
| CAN frame data optimization (shortened DLC, no padding) | §10.4.2.2 | `pci.*.encode(pad=False)` |
| Normal addressing: whole N_AI is the CAN identifier; data field is N_PCI + N_Data | §10.3.2, Tables 24–25 | `pci` (no in-band address byte), `addressing.NormalAddressing` |
| Extended / normal-fixed / mixed addressing (in-band address byte) — ROADMAP | §10.3.3–§10.3.5 | ROADMAP `it.addr.extended` |

## Timing (named only — no bus, no timer loop)

| Detail | Clause | Where in code |
|---|---|---|
| Timing parameters N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr; timeouts 1000 ms; N_Br/N_Cs performance requirements | §9.8.1, Table 21 | `timing.PARAMETERS` |
| Real timeout no later than the specified value + 50 % | §9.8.1 | `timing.TIMEOUT_TOLERANCE` |
| N_WFTmax: max consecutive FC.WAIT, local, not on the wire; 0 => CTS only | §9.7 | `timing` module note |

## Citation

Clause text was read with `iso_read` / `iso_cite` on `iso:15765-2@2016`. Canonical form, e.g.:

> ISO 15765-2:2016, Road vehicles — Diagnostic communication over Controller Area Network (DoCAN) —
> Part 2: Transport protocol and network layer services, §9.6.1 N_PCI
