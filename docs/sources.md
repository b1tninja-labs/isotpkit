# Sources

## Primary standard

- **ISO 15765-2:2016** — *Road vehicles — Diagnostic communication over Controller Area Network (DoCAN) —
  Part 2: Transport protocol and network layer services.* Every frame-format and parameter detail is grounded
  here, read through the `primarysource` `iso_*` tools on `iso:15765-2@2016`. The clause-by-clause map is
  `docs/iso-map.md`.

## Research notes (read-only)

- **GM GMLAN 11-bit addressing convention** — `volt` repository, `docs/OBD-CONFIG-DID-APPENDIX.md` (branch
  `claude/obd-config-re-887200`). Documents the GMLAN 11-bit convention: the physical **response** ID is the
  **request** ID + `0x400` (e.g. `0x24X` -> `0x64X`), and the flow/acceptance **filter** ID is the request ID
  + `0x300` (`0x24X` -> `0x54X`). Implemented in `isotpkit::addressing` (`gmlan_response_id`,
  `gmlan_filter_id`). These notes come from an authorized assessment of the owner's GM head unit and the
  Voltage OBD app; `isotpkit` carries only the addressing arithmetic, no captured data.

## Siblings (context, not dependencies)

- **`udsdiag`** — the application-layer UDS/GMLAN request/response codec that sits *above* this transport
  layer and deliberately stops at the reassembled application PDU. `isotpkit` is the layer that segments and
  reassembles that PDU across CAN frames. `isotpkit` does **not** depend on `udsdiag`.
- **`obd-bridge` / `fakeobd`** — adapters and a server, not a reusable codec. `isotpkit` is the codec they (or
  anything else needing ISO-TP) could build on; it is distinct from them.
- **`spskit`, `voltkit`** — sibling vehicle tools in the same workspace; the project layout and the ROADMAP
  obligation-matrix discipline follow `spskit`, `udsdiag` and the other Rust siblings.

## Overlap: `fakeobd/crates/isotp`

`fakeobd` already carries an ISO-TP implementation in `crates/isotp/src/lib.rs`. It is geared to the
fake-vehicle server (a `Reassembler` that owes the sender a `FlowControl`, a `CanFrame` type) and its own
ROADMAP marks its PCI nibbles, FlowStatus and STmin `[unread]` — tested round-trips, but not re-read against
§9.6. `isotpkit` is the standalone, fully-grounded, sans-I/O, `no_std` codec: typed N_PDUs with
`encode`/`decode`, a `SeparationTime`, and timing/addressing constants.

Two ISO-TP codecs in one workspace is a known duplication. The owner's stated options for resolving it:

1. **Leave `fakeobd` as it is.** Its `isotp` crate stays server-specific; `isotpkit` is the reusable one.
   Nothing cross-repo changes now.
2. **Consolidate onto `isotpkit`, with Python bindings.** `fakeobd` drops its own `isotp` and depends on
   `isotpkit`; `isotpkit` grows an optional Python binding layer (e.g. a `pyo3` feature) so the surface the
   Python package offered is not lost. This is a cross-repo migration: which API `fakeobd` adopts, how its
   `[unread]` behaviours map onto `isotpkit`'s grounded ones, and the binding build.

Option 2 is tracked as the `[open]` consolidation row in `ROADMAP.md`. It is deferred — a decision for the
owner, not taken unilaterally from this repository.
