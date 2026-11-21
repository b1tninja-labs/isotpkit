"""isotpkit command line: decode a CAN frame, or segment a message, as ISO-TP N_PDUs."""

from __future__ import annotations

import argparse
from collections.abc import Sequence
from typing import assert_never

from isotpkit import pci
from isotpkit.flowcontrol import SeparationTime
from isotpkit.segment import segment


def _hex_to_bytes(text: str) -> bytes:
    cleaned = text.replace(" ", "").replace(":", "")
    try:
        return bytes.fromhex(cleaned)
    except ValueError as exc:
        raise SystemExit(f"not a hex byte string: {text!r}") from exc


def describe(pdu: pci.NPdu) -> str:
    """A one-line human description of a decoded N_PDU."""
    match pdu:
        case pci.SingleFrame():
            return f"SingleFrame SF_DL={pdu.sf_dl} data={pdu.data.hex()}"
        case pci.FirstFrame():
            return f"FirstFrame FF_DL={pdu.length} data={pdu.data.hex()}"
        case pci.ConsecutiveFrame():
            return f"ConsecutiveFrame SN={pdu.sn} data={pdu.data.hex()}"
        case pci.FlowControl():
            stmin = SeparationTime.from_byte(pdu.stmin)
            return (
                f"FlowControl FS={pdu.flow_status.name} BS={pdu.block_size} "
                f"STmin={stmin.microseconds}us (byte={pdu.stmin:#04x})"
            )
        case _:  # pragma: no cover - exhaustiveness guard
            assert_never(pdu)


def main(argv: Sequence[str] | None = None) -> int:
    """Entry point for the ``isotpkit`` console script."""
    parser = argparse.ArgumentParser(prog="isotpkit", description="ISO-TP (ISO 15765-2) frame codec.")
    sub = parser.add_subparsers(dest="command", required=True)

    decode_parser = sub.add_parser("decode", help="decode one classic-CAN frame as an N_PDU")
    decode_parser.add_argument("frame", help="frame bytes as hex, e.g. 0211 22 or 02:11:22")

    segment_parser = sub.add_parser("segment", help="segment a message into its N_PDU frame sequence")
    segment_parser.add_argument("message", help="message bytes as hex")

    args = parser.parse_args(argv)

    if args.command == "decode":
        print(describe(pci.decode_frame(_hex_to_bytes(args.frame))))
        return 0

    for pdu in segment(_hex_to_bytes(args.message)):
        print(pdu.encode().hex())
    return 0
