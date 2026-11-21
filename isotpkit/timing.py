"""Network-layer timing parameters, named from the standard — no live timer runs here.

Grounded in ISO 15765-2:2016, 9.7 (N_WFTmax) and 9.8.1 (Timing parameters, Table 21).
There is no bus in this package, so these are named constants and their standard timeout
values for documentation and for any sender/receiver built on top, not a timer loop.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class TimingParameter:
    """One network-layer timing parameter and its standard timeout.

    ``timeout_ms`` is the Table 21 timeout where the standard fixes one, and ``None`` where
    Table 21 states a performance requirement instead of a timeout (N_Br, N_Cs).
    """

    name: str
    description: str
    timeout_ms: int | None


# ISO 15765-2:2016, Table 21 — timeouts are 1000 ms; N_Br and N_Cs carry performance
# requirements ((N_Br + N_Ar) < 0.9 x N_Bs, (N_Cs + N_As) < 0.9 x N_Cr) rather than timeouts.
N_AS = TimingParameter("N_As", "Transmission of any N_PDU on the sender side", 1000)
N_AR = TimingParameter("N_Ar", "Transmission of any N_PDU on the receiver side", 1000)
N_BS = TimingParameter("N_Bs", "Time until reception of the next FlowControl N_PDU", 1000)
N_BR = TimingParameter("N_Br", "Time until transmission of the next FlowControl N_PDU", None)
N_CS = TimingParameter("N_Cs", "Time until transmission of the next ConsecutiveFrame N_PDU", None)
N_CR = TimingParameter("N_Cr", "Time until reception of the next ConsecutiveFrame N_PDU", 1000)

PARAMETERS: tuple[TimingParameter, ...] = (N_AS, N_AR, N_BS, N_BR, N_CS, N_CR)

# "The real timeout shall occur no later than at the specified timeout value + 50 %." (9.8.1)
TIMEOUT_TOLERANCE = 0.5

# N_WFTmax (9.7): the maximum number of FC.WAIT frames a receiver may send in a row. It is
# local to the peer, not carried on the wire, and user-defined at system generation (no fixed
# default); 0 means the receiver uses FC.CTS only and never FC.WAIT.


def parameter_names() -> tuple[str, ...]:
    """The names of the six network-layer timing parameters, in Table 21 order."""
    return tuple(parameter.name for parameter in PARAMETERS)
