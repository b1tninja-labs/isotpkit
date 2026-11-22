# Instructions for coding agents

`isotpkit` is the ISO-TP (ISO 15765-2:2016, DoCAN) transport/network-layer codec, in Rust.
`PLAN.md` says what is in this slice, `ROADMAP.md` is the obligation matrix and work queue, and
`docs/iso-map.md` records the ISO clauses every frame-format detail is grounded in. This file takes
precedence over `D:\code\AGENTS.md` for work in this project.

## Commands

Run from this directory, never from `D:\code` (never a formatter or `clippy --fix` from the
workspace root):

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo doc --no-deps          # with RUSTDOCFLAGS=-D warnings
    cargo build                  # the no_std build: a std leak in the lib fails here

`python -m projecthub shells verify --rust D:/code/isotpkit` runs the gate set in one pass. CI also
builds on Rust 1.85 (the MSRV) and runs the release-profile tests.

## Invariants

1. **The matrix and the code agree.** A row is `[ok]` only when the Rust test its Test cell names
   exists and would fail if the behaviour regressed. `ROADMAP.md` is `Status | Obligation | Anchor |
   Test` (the Rust grammar `projecthub sync` reads).
2. **No frame-format detail from memory.** Every PCI nibble, length field, parameter name and
   timing value is read from ISO 15765-2:2016 through the `primarysource` `iso_*` tools first and
   cited in `docs/iso-map.md`. Do not state an ISO section or value from memory (a root `AGENTS.md`
   rule).
3. **Sans-I/O: no bus, no live timer.** No bus (socketcan, J2534) and no live timer loop. The
   `session` module paces a sender/receiver (BlockSize, STmin, WAIT, OVFLW, N_WFTmax) by *emitting*
   actions — send this frame, wait for a FlowControl, wait this STmin — for the caller to perform;
   it owns no bus and runs no timer. The timing parameters are still named constants only. Keep new
   work sans-I/O in the same way.
4. **Grounded frame scope.** Classic CAN and CAN FD (`CAN_DL` up to 64), normal/extended/mixed
   addressing, the SF and 32-bit FF escapes — all built. Frames the codec does not implement are
   rejected, not guessed. Do not widen the codec without the matching ROADMAP row, test, and a
   `primarysource` citation.
5. **No application layer and no dependency on `udsdiag`.** This crate stops at the reassembled
   application PDU bytes. Service SIDs, NRCs and DIDs belong to `udsdiag`, which sits above and does
   not depend on this one.
6. **`no_std` + `alloc`, no runtime dependencies in the core.** The core crate is `#![no_std]`; only
   `src/main.rs` uses `std`. `unsafe` is forbidden by the lint table — do not relax it in the core.
   Lints live in `Cargo.toml` `[lints]`, not scattered `#![…]` attributes.
7. **Python bindings are a separate member.** The optional pyo3 bindings live in `py/`
   (`isotpkit-py`, imported as the Python module `isotpkit`). They are `std` and allow `unsafe`
   (pyo3's macros need it) — which is exactly why they are not in the core crate. They depend on the
   core via a path dependency and add no API the core does not already expose. The default gates
   (no `--workspace`) build only the core; the bindings have their own CI job. Build/test them with
   `cd py && maturin develop && pytest`, or lint with `PYO3_PYTHON=<python> cargo clippy -p
   isotpkit-py -- -D warnings`.
8. **Overlap with `fakeobd`.** `fakeobd/crates/isotp` is a second ISO-TP implementation. Keep this
   crate standalone and fully grounded; the consolidation (fakeobd depending on `isotpkit`) is the
   `[open]` row in `ROADMAP.md`, pursued in the `fakeobd` repo, not unilaterally from here.

## Workspace rules

- Commit only inside this repository; check `git rev-parse --show-toplevel`. Never commit with `-A`.
- Write scratch output outside the repository.
- Commit messages carry no AI attribution trailer.
- Registration in projecthub/PORTFOLIO is done by the workspace-root owner. This crate is still in
  projecthub's Python/hand-owned set; the HAND_OWNED→RUST_OWNED move is handled separately.
