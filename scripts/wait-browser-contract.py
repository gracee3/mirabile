#!/usr/bin/env python3
"""Poll a local headless Chromium page over CDP using only the standard library."""

from __future__ import annotations

import base64
import json
import os
import socket
import struct
import sys
import time
import urllib.request
from urllib.parse import urlparse


def websocket_url(port: int) -> str:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(
                f"http://127.0.0.1:{port}/json/list", timeout=0.5
            ) as response:
                targets = json.load(response)
            for target in targets:
                if target.get("type") == "page":
                    return str(target["webSocketDebuggerUrl"])
        except (OSError, ValueError, KeyError):
            time.sleep(0.1)
    raise RuntimeError("Chromium page target was not available")


def connect(url: str) -> socket.socket:
    parsed = urlparse(url)
    connection = socket.create_connection((parsed.hostname, parsed.port), timeout=5)
    key = base64.b64encode(os.urandom(16)).decode("ascii")
    request = (
        f"GET {parsed.path} HTTP/1.1\r\n"
        f"Host: {parsed.hostname}:{parsed.port}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    )
    connection.sendall(request.encode("ascii"))
    response = bytearray()
    while b"\r\n\r\n" not in response:
        response.extend(connection.recv(4096))
    if not response.startswith(b"HTTP/1.1 101"):
        raise RuntimeError(f"WebSocket upgrade failed: {response[:160]!r}")
    return connection


def send_text(connection: socket.socket, value: str) -> None:
    payload = value.encode("utf-8")
    mask = os.urandom(4)
    header = bytearray([0x81])
    length = len(payload)
    if length < 126:
        header.append(0x80 | length)
    elif length < 65_536:
        header.append(0x80 | 126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(0x80 | 127)
        header.extend(struct.pack("!Q", length))
    header.extend(mask)
    masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
    connection.sendall(header + masked)


def read_exact(connection: socket.socket, length: int) -> bytes:
    output = bytearray()
    while len(output) < length:
        chunk = connection.recv(length - len(output))
        if not chunk:
            raise RuntimeError("WebSocket closed unexpectedly")
        output.extend(chunk)
    return bytes(output)


def receive_text(connection: socket.socket) -> str:
    fragments = bytearray()
    while True:
        first, second = read_exact(connection, 2)
        final = bool(first & 0x80)
        opcode = first & 0x0F
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", read_exact(connection, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", read_exact(connection, 8))[0]
        mask = read_exact(connection, 4) if second & 0x80 else None
        payload = read_exact(connection, length)
        if mask:
            payload = bytes(
                byte ^ mask[index % 4] for index, byte in enumerate(payload)
            )
        if opcode == 0x8:
            raise RuntimeError("WebSocket closed unexpectedly")
        if opcode == 0x9:
            continue
        if opcode in (0x0, 0x1):
            fragments.extend(payload)
            if final:
                return fragments.decode("utf-8")


def evaluate(connection: socket.socket, request_id: int, expression: str) -> object:
    send_text(
        connection,
        json.dumps(
            {
                "id": request_id,
                "method": "Runtime.evaluate",
                "params": {"expression": expression, "returnByValue": True},
            }
        ),
    )
    while True:
        response = json.loads(receive_text(connection))
        if response.get("id") == request_id:
            return response["result"]["result"].get("value")


def main() -> int:
    port = int(sys.argv[1])
    connection = connect(websocket_url(port))
    connection.settimeout(5)
    deadline = time.monotonic() + 30
    request_id = 1
    try:
        while time.monotonic() < deadline:
            status = evaluate(
                connection,
                request_id,
                "document.querySelector('#browser-contract-result')?.dataset.status ?? 'missing'",
            )
            request_id += 1
            if status in ("passed", "failed"):
                dom = evaluate(
                    connection,
                    request_id,
                    "document.documentElement.outerHTML",
                )
                print(dom)
                return 0 if status == "passed" else 1
            time.sleep(0.1)
    finally:
        connection.close()
    raise RuntimeError("browser contract did not reach a terminal DOM marker")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"browser contract driver failed: {error}", file=sys.stderr)
        raise SystemExit(1)
