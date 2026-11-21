"""isotpkit: an offline ISO-TP (ISO 15765-2:2016, DoCAN) transport/network-layer codec.

The package segments an application message (a UDS/GMLAN PDU) into the four ISO-TP
N_PDU types and reassembles them back, on classic-CAN 8-byte frames. It encodes and
decodes frames and runs the segmentation/reassembly state machine; it does not open a
bus or run a live timer loop (there is no bus), so the network-layer timing parameters
are named constants only.

It is the transport layer *under* the application-layer sibling ``udsdiag``, which stops
at the reassembled PDU. This package does not depend on ``udsdiag``.

Every frame-format and parameter detail is grounded in ISO 15765-2:2016; the clause map
is ``docs/iso-map.md``.
"""

from __future__ import annotations

__version__ = "0.0.0"
