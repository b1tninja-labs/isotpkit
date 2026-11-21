//! Python bindings for `isotpkit`, the ISO 15765-2:2016 (ISO-TP / DoCAN) transport-layer codec.
//!
//! The compiled module is imported as `isotpkit`. Build the wheel with maturin (see
//! `pyproject.toml`), or build the extension directly with `cargo build` and rename the cdylib to
//! `isotpkit.pyd` (Windows) / `isotpkit.so` (Unix).
//!
//! The bindings wrap the pure-Rust codec: segmentation and reassembly operate on `bytes` frames,
//! and the parameter helpers mirror the `flowcontrol` and `addressing` modules.

use isotpkit::Error;
use isotpkit::addressing::{gmlan_filter_id as core_filter, gmlan_response_id as core_response};
use isotpkit::cli::describe as core_describe;
use isotpkit::flowcontrol::{SeparationTime, encode_stmin as core_encode_stmin};
use isotpkit::pci::{NPdu, decode_frame};
use isotpkit::segment::{SegmentPdu, reassemble as core_reassemble, segment as core_segment};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[allow(
    clippy::needless_pass_by_value,
    reason = "used as a `map_err` combinator, which passes the error by value"
)]
fn to_py_err(err: Error) -> PyErr {
    PyValueError::new_err(err.to_string())
}

fn to_segment(pdu: NPdu) -> PyResult<SegmentPdu> {
    match pdu {
        NPdu::Single(frame) => Ok(SegmentPdu::Single(frame)),
        NPdu::First(frame) => Ok(SegmentPdu::First(frame)),
        NPdu::Consecutive(frame) => Ok(SegmentPdu::Consecutive(frame)),
        NPdu::Flow(_) => Err(PyValueError::new_err(
            "a FlowControl frame is not part of a reassembly",
        )),
    }
}

/// Segment an application message into its ISO-TP frames, each padded to 8 bytes.
///
/// A message of 1..=7 bytes yields one SingleFrame; a longer one yields a FirstFrame and the
/// ConsecutiveFrames. Raises `ValueError` for an empty message or one over 4095 bytes.
#[pyfunction]
fn segment<'py>(py: Python<'py>, message: &[u8]) -> PyResult<Vec<Bound<'py, PyBytes>>> {
    let pdus = core_segment(message).map_err(to_py_err)?;
    Ok(pdus
        .iter()
        .map(|pdu| PyBytes::new(py, &pdu.encode()))
        .collect())
}

/// Reassemble an ordered sequence of ISO-TP frames back into the application message.
///
/// Each frame is decoded and fed to the reassembler. Raises `ValueError` for a bad frame, a wrong
/// SequenceNumber, a FlowControl frame, or a sequence that does not complete a message.
#[pyfunction]
#[allow(
    clippy::needless_pass_by_value,
    reason = "pyo3 extracts the Python list into an owned Vec; a borrowed slice cannot be extracted"
)]
fn reassemble(py: Python<'_>, frames: Vec<Vec<u8>>) -> PyResult<Bound<'_, PyBytes>> {
    let mut pdus = Vec::with_capacity(frames.len());
    for frame in &frames {
        pdus.push(to_segment(decode_frame(frame).map_err(to_py_err)?)?);
    }
    let message = core_reassemble(&pdus).map_err(to_py_err)?;
    Ok(PyBytes::new(py, &message))
}

/// Decode one classic-CAN frame and return a one-line description of its N_PDU.
///
/// Raises `ValueError` for a reserved N_PCItype or a malformed frame.
#[pyfunction]
fn describe(frame: &[u8]) -> PyResult<String> {
    Ok(core_describe(&decode_frame(frame).map_err(to_py_err)?))
}

/// Decode an STmin byte into its minimum separation time, in microseconds.
#[pyfunction]
#[must_use]
fn stmin_microseconds(raw: u8) -> u32 {
    SeparationTime::from_byte(raw).microseconds()
}

/// Encode a minimum separation time in microseconds into an STmin byte.
///
/// Raises `ValueError` for a value that is not representable (whole ms 0..=127, or 100..=900 µs).
#[pyfunction]
fn encode_stmin(microseconds: u32) -> PyResult<u8> {
    core_encode_stmin(microseconds).map_err(to_py_err)
}

/// The GM GMLAN physical response ID for a request ID (request + 0x400).
///
/// Raises `ValueError` if the request, or the response, is outside the 11-bit range.
#[pyfunction]
fn gmlan_response_id(request_id: u32) -> PyResult<u32> {
    core_response(request_id).map_err(to_py_err)
}

/// The GM GMLAN flow/acceptance filter ID for a request ID (request + 0x300).
///
/// Raises `ValueError` if the request, or the filter ID, is outside the 11-bit range.
#[pyfunction]
fn gmlan_filter_id(request_id: u32) -> PyResult<u32> {
    core_filter(request_id).map_err(to_py_err)
}

/// The `isotpkit` Python module.
#[pymodule]
#[pyo3(name = "isotpkit")]
fn bindings(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(
        "__doc__",
        "ISO 15765-2:2016 (ISO-TP / DoCAN) transport-layer codec.",
    )?;
    m.add_function(wrap_pyfunction!(segment, m)?)?;
    m.add_function(wrap_pyfunction!(reassemble, m)?)?;
    m.add_function(wrap_pyfunction!(describe, m)?)?;
    m.add_function(wrap_pyfunction!(stmin_microseconds, m)?)?;
    m.add_function(wrap_pyfunction!(encode_stmin, m)?)?;
    m.add_function(wrap_pyfunction!(gmlan_response_id, m)?)?;
    m.add_function(wrap_pyfunction!(gmlan_filter_id, m)?)?;
    Ok(())
}
