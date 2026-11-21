"""The CLI decodes a frame and segments a message (isotpkit.cli)."""

from __future__ import annotations

import pytest

from isotpkit.cli import main


def test_cli_decode(capsys):
    assert main(["decode", "30 08 14"]) == 0
    out = capsys.readouterr().out
    assert "FlowControl" in out
    assert "FS=CONTINUE_TO_SEND" in out
    assert "BS=8" in out


def test_cli_decode_single_frame(capsys):
    assert main(["decode", "0311f190"]) == 0
    assert "SingleFrame SF_DL=3 data=11f190" in capsys.readouterr().out


def test_cli_segment(capsys):
    assert main(["segment", "00010203040506070809"]) == 0  # 10 bytes => FF + 1 CF
    lines = capsys.readouterr().out.split()
    assert lines[0] == "100a000102030405"  # FF_DL=10, first 6 bytes
    assert lines[1].startswith("21")  # CF, SN=1


def test_cli_rejects_bad_hex():
    with pytest.raises(SystemExit, match="not a hex"):
        main(["decode", "xyz"])
