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
3. **Codec, not stack.** No bus (socketcan, J2534), no live timer loop, no FlowControl pacing. The
   timing parameters are named constants only. A paced sender/receiver is a ROADMAP item, not a
   quiet addition here.
4. **Classic CAN only.** `CAN_DL` is at most 8 and addressing is normal (no in-band address byte).
   CAN FD, the SF/FF escape sequences and extended/mixed addressing are ROADMAP rows; the decoder
   rejects those frames rather than guessing. Do not widen the decoder without the matching ROADMAP
   row and test.
5. **No application layer and no dependency on `udsdiag`.** This crate stops at the reassembled
   application PDU bytes. Service SIDs, NRCs and DIDs belong to `udsdiag`, which sits above and does
   not depend on this one.
6. **`no_std` + `alloc`, no runtime dependencies.** The library is `#![no_std]`; only `src/main.rs`
   uses `std`. `unsafe` is forbidden by the lint table — do not relax it. Lints live in `Cargo.toml`
   `[lints]`, not scattered `#![…]` attributes.
7. **Overlap with `fakeobd`.** `fakeobd/crates/isotp` is a second ISO-TP implementation. Keep this
   crate standalone and fully grounded; a consolidation into one canonical crate is the `[open]`
   row in `ROADMAP.md`, a cross-repo decision not to be taken unilaterally from here.

## Workspace rules

- Commit only inside this repository; check `git rev-parse --show-toplevel`. Never commit with `-A`.
- Write scratch output outside the repository.
- Commit messages carry no AI attribution trailer.
- Registration in projecthub/PORTFOLIO is done by the workspace-root owner. This crate is still in
  projecthub's Python/hand-owned set; the HAND_OWNED→RUST_OWNED move is handled separately.
