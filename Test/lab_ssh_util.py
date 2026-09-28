#!/usr/bin/env python3
"""Paramiko helper for CPN VirtualBox lab SSH (validates host keys)."""
from __future__ import annotations

from pathlib import Path

try:
    import paramiko
except ImportError:  # pragma: no cover
    paramiko = None  # type: ignore

LAB_KNOWN_HOSTS = Path(__file__).resolve().with_name("lab_known_hosts")


def connect_cpn_lab(
    host: str = "127.0.0.1",
    port: int = 2228,
    username: str = "cpn",
    password: str = "",
    *,
    timeout: int = 30,
) -> "paramiko.SSHClient":
    if paramiko is None:
        raise RuntimeError("paramiko is not installed")
    if not LAB_KNOWN_HOSTS.is_file():
        raise RuntimeError(f"Missing lab known_hosts file: {LAB_KNOWN_HOSTS}")
    client = paramiko.SSHClient()
    client.load_host_keys(str(LAB_KNOWN_HOSTS))
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
