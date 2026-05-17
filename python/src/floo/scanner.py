"""OS-level port probing.

We never trust the registry alone — before handing out a port, we also ask the
OS whether anything is bound to it. This guards against unrelated processes
(Docker, random scripts) that floo doesn't know about.
"""
from __future__ import annotations

import socket


def is_port_free_on_os(port: int, host: str = "127.0.0.1") -> bool:
    """Return True if nothing is currently bound to (host, port).

    We test by trying to bind a socket ourselves and immediately closing it.
    If bind() raises OSError (typically EADDRINUSE), something else holds it.

    Caveat: this is a point-in-time check. Between this call and the agent
    actually starting its dev server, an unrelated process could grab the port.
    We accept that race — see design discussion. The agent will fail loudly
    with EADDRINUSE if it happens, and the user just reruns.
    """
    # SO_REUSEADDR is intentionally NOT set: we want a strict bindability check
    # that matches what most dev servers actually do.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        try:
            s.bind((host, port))
        except OSError:
            return False
    return True
