#!/usr/bin/env python3
"""Small reusable Chrome DevTools Protocol client using only the standard library."""

from __future__ import annotations

import base64
import json
import os
import socket
import struct
import time
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


class CDPError(RuntimeError):
    """A transport or protocol failure."""


def require_loopback(url: str) -> None:
    parsed = urlparse(url)
    if parsed.hostname not in {"127.0.0.1", "localhost", "::1"}:
        raise CDPError("CDP may attach only to a loopback debugging endpoint")


def websocket_url(port: int, timeout: float = 10.0) -> str:
    endpoint = f"http://127.0.0.1:{port}/json/list"
    require_loopback(endpoint)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(endpoint, timeout=0.5) as response:
                targets = json.load(response)
            for target in targets:
                if target.get("type") == "page":
                    result = str(target["webSocketDebuggerUrl"])
                    require_loopback(result)
                    return result
        except (OSError, ValueError, KeyError):
            time.sleep(0.1)
    raise CDPError("Chromium page target was not available")


def _read_exact(connection: socket.socket, length: int) -> bytes:
    output = bytearray()
    while len(output) < length:
        chunk = connection.recv(length - len(output))
        if not chunk:
            raise CDPError("WebSocket closed unexpectedly")
        output.extend(chunk)
    return bytes(output)


def _send_frame(connection: socket.socket, payload: bytes, opcode: int = 0x1) -> None:
    mask = os.urandom(4)
    header = bytearray([0x80 | opcode])
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


def _receive_text(connection: socket.socket) -> str:
    fragments = bytearray()
    while True:
        first, second = _read_exact(connection, 2)
        final = bool(first & 0x80)
        opcode = first & 0x0F
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", _read_exact(connection, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", _read_exact(connection, 8))[0]
        mask = _read_exact(connection, 4) if second & 0x80 else None
        payload = _read_exact(connection, length)
        if mask:
            payload = bytes(
                byte ^ mask[index % 4] for index, byte in enumerate(payload)
            )
        if opcode == 0x8:
            raise CDPError("WebSocket closed unexpectedly")
        if opcode == 0x9:
            _send_frame(connection, payload, opcode=0xA)
            continue
        if opcode in (0x0, 0x1):
            fragments.extend(payload)
            if final:
                return fragments.decode("utf-8")


@dataclass
class CDPClient:
    connection: socket.socket
    next_id: int = 1
    events: list[dict[str, Any]] = field(default_factory=list)

    @classmethod
    def attach(cls, port: int, timeout: float = 10.0) -> "CDPClient":
        url = websocket_url(port, timeout)
        parsed = urlparse(url)
        require_loopback(url)
        connection = socket.create_connection((parsed.hostname, parsed.port), timeout=5)
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        path = parsed.path + (f"?{parsed.query}" if parsed.query else "")
        request = (
            f"GET {path} HTTP/1.1\r\n"
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
            connection.close()
            raise CDPError(f"WebSocket upgrade failed: {response[:160]!r}")
        connection.settimeout(10)
        return cls(connection)

    def close(self) -> None:
        self.connection.close()

    def call(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        request_id = self.next_id
        self.next_id += 1
        message: dict[str, Any] = {"id": request_id, "method": method}
        if params is not None:
            message["params"] = params
        _send_frame(self.connection, json.dumps(message).encode("utf-8"))
        while True:
            response = json.loads(_receive_text(self.connection))
            if response.get("id") != request_id:
                self.events.append(response)
                continue
            if "error" in response:
                raise CDPError(f"{method} failed: {response['error']}")
            return dict(response.get("result", {}))

    def evaluate(self, expression: str) -> Any:
        result = self.call(
            "Runtime.evaluate",
            {
                "expression": expression,
                "returnByValue": True,
                "awaitPromise": True,
            },
        )
        remote = result.get("result", {})
        if remote.get("subtype") == "error" or "exceptionDetails" in result:
            raise CDPError(f"browser evaluation failed: {result}")
        return remote.get("value")

    def dispatch_key(self, key: str) -> None:
        self.call("Input.dispatchKeyEvent", {"type": "keyDown", "key": key})
        self.call("Input.dispatchKeyEvent", {"type": "keyUp", "key": key})

    def dom(self) -> str:
        value = self.evaluate("document.documentElement.outerHTML")
        if not isinstance(value, str):
            raise CDPError("DOM capture did not return text")
        return value

    def screenshot(self, path: Path) -> None:
        result: dict[str, Any] | None = None
        for attempt in range(10):
            try:
                result = self.call("Page.captureScreenshot", {"format": "png"})
                break
            except CDPError as error:
                if attempt == 9 or "Internal error" not in str(error):
                    raise
                time.sleep(0.25 * (attempt + 1))
        assert result is not None
        data = result.get("data")
        if not isinstance(data, str):
            raise CDPError("screenshot response omitted PNG data")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(data))

    def browser_log(self) -> list[dict[str, Any]]:
        return [
            event
            for event in self.events
            if event.get("method")
            in {"Runtime.consoleAPICalled", "Runtime.exceptionThrown", "Log.entryAdded"}
        ]
