#!/usr/bin/env python3
"""Constrained local Mirabile workbench control over Chromium loopback CDP."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

from cdp_client import CDPClient, CDPError

BRIDGE = "__mirabileWorkbenchV1"


def envelope(command: str, value: Any = None, error: str | None = None) -> dict[str, Any]:
    result: dict[str, Any] = {"ok": error is None, "command": command}
    if error is None:
        result["value"] = value
    else:
        result["error"] = error
    return result


def bridge_call(client: CDPClient, method: str, *arguments: Any) -> dict[str, Any]:
    if method not in {
        "controls",
        "execute",
        "replayMacro",
        "peerInitialize",
        "peerReplayMacro",
        "peerSnapshot",
        "setActionSource",
        "snapshot",
        "trace",
    }:
        raise CDPError(f"bridge method {method!r} is not whitelisted")
    args = ",".join(json.dumps(argument) for argument in arguments)
    expression = (
        f"window.{BRIDGE}?.{method}({args}) ?? "
        + json.dumps(
            json.dumps(
                {
                    "ok": False,
                    "kind": method,
                    "error": "Mirabile automation bridge is unavailable",
                }
            )
        )
    )
    raw = client.evaluate(expression)
    if not isinstance(raw, str):
        raise CDPError(f"bridge method {method} did not return a JSON envelope")
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise CDPError(f"bridge method {method} returned a non-object envelope")
    return value


def bridge_settled(client: CDPClient) -> bool:
    return bool(client.evaluate(f"window.{BRIDGE}?.waitSettled() ?? false"))


def peer_settled(client: CDPClient) -> bool:
    return bool(client.evaluate(f"window.{BRIDGE}?.peerWaitSettled() ?? false"))


def controls(client: CDPClient) -> list[dict[str, Any]]:
    response = bridge_call(client, "controls")
    if not response.get("ok"):
        raise CDPError(str(response.get("error", "control manifest failed")))
    value = response.get("value", {})
    if not isinstance(value, dict) or not isinstance(value.get("controls"), list):
        raise CDPError("control manifest envelope was malformed")
    return list(value["controls"])


def get_control(client: CDPClient, address: str) -> dict[str, Any]:
    available = controls(client)
    matches = [
        control
        for control in available
        if canonical_address(control.get("address")) == address
    ]
    if not matches and "[" not in address:
        matches = [
            control
            for control in available
            if isinstance(control.get("address"), dict)
            and control["address"].get("control") == address
        ]
    if len(matches) != 1:
        raise CDPError(
            f"expected exactly one control at {address!r}; found {len(matches)}"
        )
    return matches[0]


def canonical_address(value: Any) -> str:
    if not isinstance(value, dict) or not isinstance(value.get("control"), str):
        return ""
    qualifiers = value.get("qualifiers", {})
    if not isinstance(qualifiers, dict) or not qualifiers:
        return value["control"]
    suffix = ",".join(f"{key}={qualifiers[key]}" for key in sorted(qualifiers))
    return f"{value['control']}[{suffix}]"


def native_expression(address: str, operation: str, value: Any = None) -> str:
    address_json = json.dumps(address)
    value_json = json.dumps(value)
    operations = {
        "click": "native.click(); return true;",
        "focus": "native.focus(); return true;",
        "set": (
            "if (!('value' in native)) throw new Error('control has no value');"
            f"native.value = {value_json};"
            "native.dispatchEvent(new InputEvent('input', {bubbles:true, inputType:'insertText'}));"
            "native.dispatchEvent(new Event('change', {bubbles:true})); return true;"
        ),
        "select": (
            "if (native.tagName !== 'SELECT') throw new Error('control is not a select');"
            f"native.value = {value_json};"
            "native.dispatchEvent(new Event('change', {bubbles:true})); return true;"
        ),
        "check": (
            "if (native.type !== 'checkbox') throw new Error('control is not a checkbox');"
            f"native.checked = {str(bool(value)).lower()};"
            "native.dispatchEvent(new Event('change', {bubbles:true})); return true;"
        ),
    }
    if operation not in operations:
        raise CDPError(f"native operation {operation!r} is not whitelisted")
    return (
        "(() => {"
        f"const address={address_json};"
        "const control=[...document.querySelectorAll('[data-mirabile-address]')]"
        ".find(candidate => candidate.dataset.mirabileAddress === address);"
        "if (!control) throw new Error(`control not found: ${address}`);"
        "const native=control.matches('button,input,select,textarea') ? control : "
        "control.querySelector('[data-mirabile-native=\"value\"]');"
        "if (!native) throw new Error(`native control target missing: ${address}`);"
        f"{operations[operation]}"
        "})()"
    )


def wait_settled(client: CDPClient, timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if bridge_settled(client):
            return bridge_call(client, "snapshot")
        time.sleep(0.05)
    raise CDPError(f"workbench did not settle within {timeout:.1f}s")


def wait_peer_settled(client: CDPClient, timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if peer_settled(client):
            return bridge_call(client, "peerSnapshot")
        time.sleep(0.05)
    raise CDPError(f"peer workbench did not settle within {timeout:.1f}s")


def load_json(value: str) -> Any:
    path = Path(value)
    text = path.read_text(encoding="utf-8") if path.is_file() else value
    return json.loads(text)


def perform(client: CDPClient, command: str, options: dict[str, Any]) -> Any:
    if command in {"snapshot", "controls", "trace"}:
        return bridge_call(client, command)
    if command == "get":
        return get_control(client, str(options["address"]))
    if command == "discover":
        manifest_controls = controls(client)
        control_id = str(options["control"])
        qualifiers = options.get("qualifiers", {})
        matches = [control for control in manifest_controls if control.get("address", {}).get("control") == control_id and all(control.get("address", {}).get("qualifiers", {}).get(key) == value for key, value in qualifiers.items())]
        index = options.get("index")
        if index is None and len(matches) != 1:
            raise CDPError(f"semantic discovery expected one {control_id!r} control, found {len(matches)}")
        if index is not None and (not isinstance(index, int) or index < 0 or index >= len(matches)):
            raise CDPError(f"semantic discovery index {index!r} is outside {len(matches)} {control_id!r} matches")
        result = dict(matches[0 if index is None else index])
        result["address"] = canonical_address(result["address"])
        return result
    if command in {"click", "set", "select", "check"}:
        address = str(options["address"])
        address = canonical_address(get_control(client, address).get("address"))
        value = options.get("value")
        return client.evaluate(native_expression(address, command, value))
    if command == "key":
        address = str(options["address"])
        address = canonical_address(get_control(client, address).get("address"))
        client.evaluate(native_expression(address, "focus"))
        client.dispatch_key(str(options["key"]))
        return True
    if command == "execute":
        request = options["request"]
        source = str(options.get("source", "agent"))
        source_result = bridge_call(client, "setActionSource", source)
        if not source_result.get("ok"):
            raise CDPError(str(source_result.get("error")))
        return bridge_call(client, "execute", json.dumps(request, separators=(",", ":")))
    if command == "replay":
        macro = options["macro"]
        return bridge_call(client, "replayMacro", json.dumps(macro, separators=(",", ":")))
    if command == "peer_initialize":
        return bridge_call(client, "peerInitialize")
    if command == "peer_replay":
        macro = options["macro"]
        return bridge_call(client, "peerReplayMacro", json.dumps(macro, separators=(",", ":")))
    if command == "peer_snapshot":
        return bridge_call(client, "peerSnapshot")
    if command == "peer_wait":
        return wait_peer_settled(client, float(options.get("timeout", 10.0)))
    if command == "wait":
        return wait_settled(client, float(options.get("timeout", 10.0)))
    if command == "dom":
        return client.dom()
    if command == "screenshot":
        path = Path(str(options["path"]))
        client.screenshot(path)
        return {"path": str(path)}
    if command == "reload":
        client.evaluate("location.reload()")
        return {"reloaded": True}
    raise CDPError(f"unknown command {command!r}")


def run_scenario(
    client: CDPClient, scenario: dict[str, Any], artifacts: Path | None
) -> dict[str, Any]:
    steps = scenario.get("steps")
    if not isinstance(steps, list):
        raise CDPError("scenario requires a steps array")
    results = []
    bindings: dict[str, Any] = {}
    try:
        for index, step in enumerate(steps):
            if not isinstance(step, dict) or not isinstance(step.get("command"), str):
                raise CDPError(f"scenario step {index + 1} is malformed")
            resolved = substitute_bindings(step, bindings)
            command = resolved["command"]
            result = perform(client, command, resolved)
            assert_expectations(result, resolved.get("expect"), index + 1)
            if isinstance(resolved.get("bind"), str):
                bindings[resolved["bind"]] = result_path(result, str(resolved.get("bind_path", "address")))
            results.append({"step": index + 1, "command": command, "value": result})
        return {"scenario": scenario.get("name", "unnamed"), "steps": results}
    except Exception as error:
        if artifacts is not None:
            collect_failure_artifacts(client, artifacts, scenario, str(error))
        raise


def result_path(value: Any, path: str) -> Any:
    current = value
    for segment in path.split(".") if path else []:
        if isinstance(current, dict) and segment in current:
            current = current[segment]
        else:
            raise CDPError(f"binding result path {path!r} was not found")
    return current


def substitute_bindings(value: Any, bindings: dict[str, Any]) -> Any:
    if isinstance(value, dict):
        return {key: substitute_bindings(item, bindings) for key, item in value.items()}
    if isinstance(value, list):
        return [substitute_bindings(item, bindings) for item in value]
    if isinstance(value, str):
        for name, bound in bindings.items():
            value = value.replace("${" + name + "}", str(bound))
    return value


def assert_expectations(result: Any, expectations: Any, step: int) -> None:
    if expectations is None:
        return
    if not isinstance(expectations, dict):
        raise CDPError(f"scenario step {step} expect must be an object")
    for path, expected in expectations.items():
        if not isinstance(path, str) or not path:
            raise CDPError(f"scenario step {step} expectation path is invalid")
        actual = result
        for segment in path.split("."):
            if isinstance(actual, dict) and segment in actual:
                actual = actual[segment]
            elif isinstance(actual, list) and segment.isdigit() and int(segment) < len(actual):
                actual = actual[int(segment)]
            else:
                raise CDPError(
                    f"scenario step {step} expectation path {path!r} was not present"
                )
        if actual != expected:
            raise CDPError(
                f"scenario step {step} expected {path}={expected!r}; got {actual!r}"
            )


def collect_failure_artifacts(
    client: CDPClient, directory: Path, scenario: dict[str, Any], error: str
) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    client.screenshot(directory / "screenshot.png")
    (directory / "dom.html").write_text(client.dom(), encoding="utf-8")
    for filename, method in (
        ("controls.json", "controls"),
        ("application.json", "snapshot"),
        ("trace.json", "trace"),
    ):
        (directory / filename).write_text(
            json.dumps(bridge_call(client, method), indent=2, sort_keys=True),
            encoding="utf-8",
        )
    (directory / "browser.log").write_text(
        json.dumps(client.browser_log(), indent=2, sort_keys=True), encoding="utf-8"
    )
    (directory / "scenario.json").write_text(
        json.dumps(scenario, indent=2, sort_keys=True), encoding="utf-8"
    )
    (directory / "error.txt").write_text(error + "\n", encoding="utf-8")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    root.add_argument("--port", type=int, required=True)
    subcommands = root.add_subparsers(dest="command", required=True)
    for command in ("snapshot", "controls", "trace", "dom"):
        subcommands.add_parser(command)
    get = subcommands.add_parser("get")
    get.add_argument("address")
    for command in ("click",):
        item = subcommands.add_parser(command)
        item.add_argument("address")
    for command in ("set", "select"):
        item = subcommands.add_parser(command)
        item.add_argument("address")
        item.add_argument("value")
    check = subcommands.add_parser("check")
    check.add_argument("address")
    check.add_argument("value", choices=("true", "false"))
    key = subcommands.add_parser("key")
    key.add_argument("address")
    key.add_argument("key")
    execute = subcommands.add_parser("execute")
    execute.add_argument("request")
    execute.add_argument("--source", default="agent")
    wait = subcommands.add_parser("wait")
    wait.add_argument("--timeout", type=float, default=10.0)
    screenshot = subcommands.add_parser("screenshot")
    screenshot.add_argument("path")
    subcommands.add_parser("reload")
    run = subcommands.add_parser("run")
    run.add_argument("scenario")
    run.add_argument("--artifacts")
    return root


def main() -> int:
    arguments = parser().parse_args()
    client = CDPClient.attach(arguments.port)
    try:
        command = arguments.command
        options = vars(arguments).copy()
        if command == "execute":
            options["request"] = load_json(arguments.request)
        elif command == "check":
            options["value"] = arguments.value == "true"
        if command == "run":
            scenario = load_json(arguments.scenario)
            if not isinstance(scenario, dict):
                raise CDPError("scenario must be a JSON object")
            value = run_scenario(
                client,
                scenario,
                Path(arguments.artifacts) if arguments.artifacts else None,
            )
        else:
            value = perform(client, command, options)
        print(json.dumps(envelope(command, value), sort_keys=True))
        return 0
    except Exception as error:
        print(json.dumps(envelope(arguments.command, error=str(error)), sort_keys=True))
        return 1
    finally:
        client.close()


if __name__ == "__main__":
    raise SystemExit(main())
