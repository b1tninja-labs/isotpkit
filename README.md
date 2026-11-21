# isotpkit

An offline **ISO-TP** codec — the transport/network layer of ISO 15765-2:2016 (DoCAN) — that segments a
UDS/GMLAN application message across CAN frames and reassembles it. It is the transport layer *under* the
application-layer sibling [`udsdiag`](../udsdiag), which stops at the reassembled PDU. `isotpkit` does not
depend on `udsdiag`, and it is distinct from the `obd-bridge`/`fakeobd` adapters: this is a reusable codec,
not a server.

It encodes and decodes the four N_PDU types, runs the segmentation/reassembly state machine, and names the
network-layer timing parameters. There is no bus here, so there is no live timer loop — the timing
parameters are named constants only.

## What it does

- **The four N_PDU types** (classic CAN, 8-byte frames, normal addressing): SingleFrame, FirstFrame,
  ConsecutiveFrame, FlowControl — each encodes and decodes (`isotpkit.pci`).
- **Segmentation**: a message of 1..7 bytes becomes one SingleFrame; a longer one becomes a FirstFrame
  (12-bit length + first 6 bytes) and ConsecutiveFrames whose SequenceNumber starts at 1 and wraps
  `0xF -> 0x0`. **Reassembly** validates SequenceNumbers (`isotpkit.segment`).
- **FlowControl parameters**: FlowStatus (CTS/WAIT/OVFLW), BlockSize, and STmin including the 0xF1..0xF9
  microsecond encoding, as a typed structure (`isotpkit.flowcontrol`).
- **Timing parameters** N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr and N_WFTmax, named from the standard with their
  Table 21 timeout values (`isotpkit.timing`).
- **11-bit normal addressing** and the GM GMLAN `+0x400` response / `+0x300` filter convention
  (`isotpkit.addressing`).

```
python -m isotpkit decode 30 08 14
python -m isotpkit decode 0311f190
python -m isotpkit segment 00010203040506070809
```

## Grounding

Every frame-format and parameter detail is grounded in ISO 15765-2:2016, read through the `primarysource`
`iso_*` tools. The clause map is [`docs/iso-map.md`](docs/iso-map.md); `docs/sources.md` records the sibling
and research sources.

CAN FD (CAN_DL > 8), the SingleFrame escape (SF_DL > 7), the 32-bit FirstFrame escape (FF_DL > 4095), and
extended/mixed addressing are planned; see [`ROADMAP.md`](ROADMAP.md). The decoder rejects those frames
rather than guessing.

Standard library only. Apache-2.0.
