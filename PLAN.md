# isotpkit: plan

The ISO-TP (ISO 15765-2:2016, DoCAN) transport/network layer, as a reusable offline codec. Registered
2026-11-21, untiered and unsequenced. It sits beside `udsdiag`, `voltkit`, `spskit` and the `volt` research
notes. It is the transport layer that segments and reassembles a UDS/GMLAN application PDU across CAN frames;
`udsdiag` is the application layer above it and deliberately stops at the reassembled PDU.

## 1. What this slice does

- Encodes and decodes the four N_PDU types on classic-CAN 8-byte frames with normal addressing: SingleFrame,
  FirstFrame (12-bit FF_DL), ConsecutiveFrame (SequenceNumber), FlowControl (`isotpkit.pci`).
- Segments an application message into its N_PDU sequence and reassembles it, validating SequenceNumbers and
  wrapping `0xF -> 0x0` (`isotpkit.segment`). Pure segmentation — no FlowControl pacing, which needs a bus.
- Models the FlowControl parameters: FlowStatus (CTS/WAIT/OVFLW), BlockSize, and the STmin byte including the
  0xF1..0xF9 microsecond encoding and the reserved-value 127 ms fallback (`isotpkit.flowcontrol`).
- Names the network-layer timing parameters N_As..N_Cr and N_WFTmax with their Table 21 timeouts, as
  constants only — there is no bus, so no timer loop runs (`isotpkit.timing`).
- Provides an 11-bit normal-addressing pair and the GM GMLAN `+0x400`/`+0x300` convention
  (`isotpkit.addressing`), read from the `volt` OBD-config appendix.
- A small CLI: `isotpkit decode <hex>` and `isotpkit segment <hex>` (`isotpkit.cli`).

## 2. Boundaries

- **No bus.** This is a codec, not a stack. No socketcan, no J2534, no live timers. A sender/receiver that
  paces ConsecutiveFrames with FlowControl and the N_* timers is a ROADMAP item that belongs to whoever
  wires a bus underneath.
- **No application layer.** Service SIDs, NRCs, DIDs and the UDS/GMLAN request/response codec are `udsdiag`'s
  job. `isotpkit` stops at the reassembled application PDU bytes.
- **Classic CAN only, for now.** CAN FD, the SF/FF escape sequences and extended/mixed addressing are in
  `ROADMAP.md`; the decoder rejects those frames rather than guessing their fields.

## 3. Grounding discipline

Every PCI nibble, length field and parameter name is read from ISO 15765-2:2016 through the `primarysource`
`iso_*` tools before it is written into the code. The clause map is `docs/iso-map.md`. No frame-format
detail is stated from memory (an `AGENTS.md` rule).
