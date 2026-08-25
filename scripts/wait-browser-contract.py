#!/usr/bin/env python3
"""Wait for the low-level browser contract's explicit terminal marker."""

from __future__ import annotations

import sys
import time

from cdp_client import CDPClient


def main() -> int:
    client = CDPClient.attach(int(sys.argv[1]))
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            status = client.evaluate(
                "document.querySelector('#browser-contract-result')?.dataset.status ?? 'missing'"
            )
            if status in ("passed", "failed"):
                print(client.dom())
                return 0 if status == "passed" else 1
            time.sleep(0.1)
    finally:
        client.close()
    raise RuntimeError("browser contract did not reach a terminal DOM marker")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"browser contract driver failed: {error}", file=sys.stderr)
        raise SystemExit(1)
