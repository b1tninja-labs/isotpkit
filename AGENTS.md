# Instructions for coding agents

`isotpkit` is the ISO-TP (ISO 15765-2:2016, DoCAN) transport/network-layer codec. `PLAN.md` says what is in
this slice, `ROADMAP.md` is the obligation matrix and work queue, and `docs/iso-map.md` records the ISO
clauses every frame-format detail is grounded in. This file takes precedence over `D:\code\AGENTS.md` for
work in this project.

## Commands

Run from this directory, never from `D:\code`:

    python -m pytest -q
    python -m ruff format isotpkit tests
    python -m ruff check isotpkit tests
    python -m mypy isotpkit
    python -O -m pytest -q

Use the pinned toolchain in `projecthub/src/projecthub/shells/requirements-dev.txt` when it is installed.

## Invariants

1. **The matrix and the code agree.** A row is `[ok]` only when it names a callable and a test that exists.
   `tests/test_roadmap_rows.py` checks it.
2. **No frame-format detail from memory.** Every PCI nibble, length field, parameter name and timing value is
   read from ISO 15765-2:2016 through the `primarysource` `iso_*` tools first and cited in `docs/iso-map.md`.
   Do not state an ISO section or value from memory (a root `AGENTS.md` rule).
3. **Codec, not stack.** No bus (socketcan, J2534), no live timer loop, no FlowControl pacing. The timing
   parameters are named constants only. A paced sender/receiver is a ROADMAP item, not a quiet addition here.
4. **Classic CAN only.** CAN_DL is at most 8 and addressing is normal (no in-band address byte). CAN FD, the
   SF/FF escape sequences and extended/mixed addressing are ROADMAP rows; the decoder rejects those frames
   rather than guessing. Do not widen the decoder without the matching ROADMAP row and test.
5. **No application layer and no dependency on `udsdiag`.** This package stops at the reassembled application
   PDU bytes. Service SIDs, NRCs and DIDs belong to `udsdiag`, which sits above and does not import this one.
6. **Standard library only.** No runtime dependencies.
