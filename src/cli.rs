//! Command-line surface: decode a CAN frame, or segment a message, as ISO-TP N_PDUs.
//!
//! This module is sans-I/O: [`run`] takes the arguments and returns the text to print, so the
//! binary (`src/main.rs`) owns all the actual input and output.

use core::fmt;

use alloc::string::{String, ToString as _};
use alloc::vec::Vec;

use crate::error::Error;
use crate::flowcontrol::SeparationTime;
use crate::pci::{NPdu, decode_frame};
use crate::segment::segment;

const USAGE: &str = "usage: isotpkit <decode|segment> <hex>\n  \
    decode <frame>     decode one classic-CAN frame as an N_PDU\n  \
    segment <message>  segment a message into its N_PDU frame sequence";

/// An error from running the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    /// Wrong or missing arguments; the payload is the usage message.
    Usage(String),
    /// An operand was not a hex byte string.
    BadHex(String),
    /// The underlying codec rejected the frame or message.
    Codec(Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(msg) => f.write_str(msg),
            CliError::BadHex(text) => write!(f, "not a hex byte string: {text:?}"),
            CliError::Codec(err) => write!(f, "{err}"),
        }
    }
}

impl core::error::Error for CliError {}

impl From<Error> for CliError {
    fn from(err: Error) -> CliError {
        CliError::Codec(err)
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(char::from(HEX[usize::from(b >> 4)]));
        s.push(char::from(HEX[usize::from(b & 0x0F)]));
    }
    s
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn from_hex(text: &str) -> Result<Vec<u8>, CliError> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    let bytes = cleaned.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(CliError::BadHex(text.to_string()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = hex_value(pair[0]).ok_or_else(|| CliError::BadHex(text.to_string()))?;
        let lo = hex_value(pair[1]).ok_or_else(|| CliError::BadHex(text.to_string()))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

/// A one-line human description of a decoded N_PDU.
#[must_use]
pub fn describe(pdu: &NPdu) -> String {
    match pdu {
        NPdu::Single(sf) => {
            alloc::format!(
                "SingleFrame SF_DL={} data={}",
                sf.sf_dl(),
                to_hex(sf.data())
            )
        }
        NPdu::First(ff) => {
            alloc::format!(
                "FirstFrame FF_DL={} data={}",
                ff.length(),
                to_hex(ff.data())
            )
        }
        NPdu::Consecutive(cf) => {
            alloc::format!("ConsecutiveFrame SN={} data={}", cf.sn(), to_hex(cf.data()))
        }
        NPdu::Flow(fc) => {
            let st = SeparationTime::from_byte(fc.stmin);
            alloc::format!(
                "FlowControl FS={} BS={} STmin={}us (byte={:#04x})",
                fc.flow_status.name(),
                fc.block_size,
                st.microseconds(),
                fc.stmin
            )
        }
    }
}

/// Run the command line on `args` (the subcommand and its operands, no program name).
///
/// Returns the text to print on success.
///
/// # Errors
/// [`CliError::Usage`] for a missing or unknown subcommand, [`CliError::BadHex`] for a bad hex
/// operand, and [`CliError::Codec`] when the codec rejects the frame or message.
pub fn run(args: &[&str]) -> Result<String, CliError> {
    match args {
        ["decode", frame] => Ok(describe(&decode_frame(&from_hex(frame)?)?)),
        ["segment", message] => {
            let pdus = segment(&from_hex(message)?)?;
            let lines: Vec<String> = pdus.iter().map(|pdu| to_hex(&pdu.encode())).collect();
            Ok(lines.join("\n"))
        }
        _ => Err(CliError::Usage(USAGE.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowcontrol::FlowStatus;
    use crate::pci::FlowControl;

    #[test]
    fn cli_decode() {
        assert_eq!(
            run(&["decode", "02 3e 00"]).unwrap(),
            "SingleFrame SF_DL=2 data=3e00"
        );
    }

    #[test]
    fn cli_segment() {
        // Two bytes become one SingleFrame, padded to 8.
        assert_eq!(run(&["segment", "3e00"]).unwrap(), "023e00cccccccccc");
    }

    #[test]
    fn describe_flow_control() {
        let fc = NPdu::Flow(FlowControl::new(FlowStatus::ContinueToSend, 0, 0xF1));
        assert_eq!(
            describe(&fc),
            "FlowControl FS=CTS BS=0 STmin=100us (byte=0xf1)"
        );
    }

    #[test]
    fn usage_and_bad_hex() {
        assert!(matches!(run(&[]), Err(CliError::Usage(_))));
        assert!(matches!(run(&["decode", "zz"]), Err(CliError::BadHex(_))));
        assert!(matches!(run(&["decode", "abc"]), Err(CliError::BadHex(_))));
    }
}
