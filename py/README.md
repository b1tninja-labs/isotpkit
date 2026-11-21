# isotpkit (Python bindings)

Python bindings for [`isotpkit`](../), the ISO 15765-2:2016 (ISO-TP / DoCAN) transport-layer codec.
The codec is implemented in Rust; this package exposes it to Python via [pyo3](https://pyo3.rs).

## Build

From this `py/` directory, with [maturin](https://www.maturin.rs):

```
pip install maturin
maturin develop        # build and install into the active environment
pytest                 # run tests/test_isotpkit.py
```

`maturin build --out dist` produces an abi3 wheel (CPython 3.9+).

## Use

```python
import isotpkit

frames = isotpkit.segment(b"\x22\xf1\x90 a UDS response payload")  # list[bytes]
message = isotpkit.reassemble(frames)                              # bytes

isotpkit.describe(bytes.fromhex("300814"))   # 'FlowControl FS=CTS BS=8 STmin=20000us (byte=0x14)'
isotpkit.stmin_microseconds(0xF1)            # 100
isotpkit.encode_stmin(100)                   # 0xF1
isotpkit.gmlan_response_id(0x241)            # 0x641
isotpkit.gmlan_filter_id(0x241)              # 0x541
```

Invalid frames, messages and parameters raise `ValueError`.
