//! Integration tests use only the public API, as a downstream user would.

use isotpkit::pci::{NPdu, decode_frame};
use isotpkit::segment::{reassemble, segment};

#[test]
fn public_api_names_the_crate() {
    assert_eq!(isotpkit::name(), "isotpkit");
}

#[test]
fn a_multi_frame_message_round_trips() {
    let message = b"\x22\xf1\x90 a UDS ReadDataByIdentifier response payload";
    let pdus = segment(message).unwrap();
    assert!(
        pdus.len() > 1,
        "this message should need more than one frame"
    );
    assert_eq!(reassemble(&pdus).unwrap(), message);
}

#[test]
fn a_decoded_single_frame_is_a_single_frame() {
    // 0x0 high nibble = SingleFrame, SF_DL = 3, three data bytes, then padding.
    let frame = [0x03, 0x7F, 0x22, 0x31, 0xCC, 0xCC, 0xCC, 0xCC];
    match decode_frame(&frame).unwrap() {
        NPdu::Single(sf) => assert_eq!(sf.data(), &[0x7F, 0x22, 0x31]),
        other => panic!("expected a SingleFrame, got {other:?}"),
    }
}
