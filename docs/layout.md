# Crate layout

A small workspace. The root crate `isotpkit` is the codec; the member `py/` (`isotpkit-py`) is the
optional pyo3 Python bindings.

- The root crate: `Cargo.toml`, `src/` (the modules + `lib.rs` + `main.rs`), `tests/`. It is
  `#![no_std]` + `alloc`, `unsafe` forbidden, no runtime dependencies.
- `py/`: the Python bindings crate. `std`, `unsafe` allowed (pyo3's macros need it), depends on the
  root crate by path. Imported as the Python module `isotpkit`. Built with maturin.
- The default build (no `--workspace`) is the root crate only, so the core gates need no Python
  toolchain; the bindings have their own CI job. `cargo fmt` still spans the whole workspace.
- Integration tests live in `tests/` and use only the public API. Unit tests sit beside the code in
  `#[cfg(test)]` modules.
- Lints are configured in each manifest's `[lints]`, not with `#![...]` attributes scattered in
  files (the few local `#[expect]`/`#[allow]` on single items are per-item, not crate config).
