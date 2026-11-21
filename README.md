# isotpkit

An offline **ISO-TP** codec in Rust — the transport/network layer of ISO 15765-2:2016 (DoCAN) —
that segments a UDS/GMLAN application message across CAN frames and reassembles it. It is the
transport layer *under* an application-layer UDS codec such as the sibling `udsdiag`, which stops
at the reassembled PDU. `isotpkit` does not depend on `udsdiag`, and it is distinct from the
`obd-bridge`/`fakeobd` adapters: this is a reusable codec, not a server.

It encodes and decodes the four N_PDU types, runs the segmentation/reassembly state machine, and
names the network-layer timing parameters. It is *sans-I/O*: no bus, no sockets, no live timer
loop — the timing parameters are named constants only. The library is `#![no_std]` (with `alloc`);
only the command-line binary uses `std`.

## What it does

- **The four N_PDU types** (classic CAN, 8-byte frames, normal addressing): `SingleFrame`,
  `FirstFrame`, `ConsecutiveFrame`, `FlowControl` — each encodes and decodes (`isotpkit::pci`).
- **Segmentation**: a message of 1..=7 bytes becomes one `SingleFrame`; a longer one becomes a
  `FirstFrame` (12-bit length + first 6 bytes) and `ConsecutiveFrame`s whose `SequenceNumber`
  starts at 1 and wraps `0xF → 0x0`. **Reassembly** validates `SequenceNumber`s
  (`isotpkit::segment`).
- **FlowControl parameters**: `FlowStatus` (CTS/WAIT/OVFLW), `BlockSize`, and `STmin` including the
  `0xF1..=0xF9` microsecond encoding and the reserved-value 127 ms fallback
  (`isotpkit::flowcontrol`).
- **Timing parameters** N_As, N_Ar, N_Bs, N_Br, N_Cs, N_Cr and N_WFTmax, named from the standard
  with their Table 21 timeout values (`isotpkit::timing`).
- **11-bit normal addressing** and the GM GMLAN `+0x400` response / `+0x300` filter convention
  (`isotpkit::addressing`).

## Library

```rust
use isotpkit::segment::{segment, reassemble};
use isotpkit::pci::decode_frame;

let message = b"\x22\xf1\x90 a UDS response payload";
let pdus = segment(message).unwrap();          // FirstFrame + ConsecutiveFrames
assert_eq!(reassemble(&pdus).unwrap(), message);

let pdu = decode_frame(&[0x02, 0x3e, 0x00]).unwrap();   // a SingleFrame
```

## Command line

```
cargo run -- decode "30 08 14"       # FlowControl FS=CTS BS=8 STmin=20000us (byte=0x14)
cargo run -- decode "02 3e 00"       # SingleFrame SF_DL=2 data=3e00
cargo run -- segment "22f190 68656c6c6f20776f726c64"
```

## Python bindings

An optional Python extension (the `isotpkit-py` workspace member, under `py/`) exposes the codec to
Python via [pyo3](https://pyo3.rs). The core crate stays pure `no_std` with `unsafe` forbidden; the
bindings are a separate crate (`std`, pyo3). Build with [maturin](https://www.maturin.rs):

```
cd py && maturin develop && pytest
```

```python
import isotpkit
frames = isotpkit.segment(b"\x22\xf1\x90 payload")   # list[bytes]
assert isotpkit.reassemble(frames) == b"\x22\xf1\x90 payload"
```

See [`py/README.md`](py/README.md) for the full surface.

## Grounding

Every frame-format and parameter detail is grounded in ISO 15765-2:2016, read through the
`primarysource` `iso_*` tools. The clause map is [`docs/iso-map.md`](docs/iso-map.md);
[`docs/sources.md`](docs/sources.md) records the sibling and research sources, including the overlap
with `fakeobd`'s own ISO-TP crate.

CAN FD (`CAN_DL` > 8), the `SingleFrame` escape (`SF_DL` > 7), the 32-bit `FirstFrame` escape
(`FF_DL` > 4095), and extended/mixed addressing are planned; see [`ROADMAP.md`](ROADMAP.md). The
decoder rejects those frames rather than guessing.

No runtime dependencies. Edition 2024, MSRV 1.85. Apache-2.0.
