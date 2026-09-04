#!/usr/bin/env python3
"""Shared Paramiko helper for CPN lab scripts (RejectPolicy + known_hosts)."""

from __future__ import annotations

import os
from typing import Optional

import paramiko

_HERE = os.path.dirname(os.path.abspath(__file__))
_DEFAULT_KNOWN_HOSTS = os.path.join(_HERE, "lab_known_hosts")


def connect(
    host: str = "127.0.0.1",
    port: int = 2222,
    username: str = "cpn",
    password: Optional[str] = None,
    timeout: int = 20,
    known_hosts: Optional[str] = None,
) -> paramiko.SSHClient:
    """Open an SSH session only when the host key is already trusted.

    Uses ``RejectPolicy`` (never AutoAdd). Operators must seed
    ``to-do/lab_known_hosts`` (for example via ``ssh-keyscan -p PORT HOST``).
    """
    client = paramiko.SSHClient()
    client.load_system_host_keys()
    path = known_hosts or os.environ.get("CPN_LAB_KNOWN_HOSTS", _DEFAULT_KNOWN_HOSTS)
    if path and os.path.isfile(path):
        client.load_host_keys(path)
    client.set_missing_host_key_policy(paramiko.RejectPolicy())
    client.connect(
        host,
        port=port,
        username=username,
        password=password,
        timeout=timeout,
        allow_agent=False,
        look_for_keys=False,
    )
    return client
